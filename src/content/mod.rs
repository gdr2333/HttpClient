//! `HttpContent` trait + concrete content types.
//!
//! Modeled on C#'s `System.Net.Http.HttpContent` and its subtypes.

use std::pin::Pin;

use ::bytes::Bytes;
use async_trait::async_trait;
use tokio::io::AsyncWrite;

use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;

pub mod byte_array;
pub mod form;
pub mod multipart;
pub mod stream;
pub mod string;

pub use byte_array::ByteArrayContent;
pub use form::FormUrlEncodedContent;
pub use multipart::{MultipartContent, MultipartFormDataContent, MultipartPart};
pub use stream::StreamContent;
pub use string::StringContent;

/// The body of an HTTP request or response, plus the headers that describe
/// it (`Content-Type`, `Content-Length`, ...). Mirrors C#'s `HttpContent`.
///
/// All methods take `&mut self` because bodies may be consumed (read
/// forward-once) or buffer into shared state, and we want the future
/// returned to be `Send` without requiring the body to be `Sync`.
#[async_trait]
pub trait HttpContent: Send {
    /// Headers associated with this body.
    fn headers(&self) -> &HttpContentHeaders;

    /// `Content-Length`, if known ahead of time.
    fn content_length(&self) -> Option<u64>;

    /// Write the body to `writer`. Implementations should NOT write a
    /// `Transfer-Encoding: chunked` framing — that is the transport's job.
    async fn write_to(
        &mut self,
        writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException>;

    /// Buffer the entire body into memory and return it.
    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        let mut buf = Vec::new();
        {
            struct VecWriter<'a>(&'a mut Vec<u8>);
            impl<'a> tokio::io::AsyncWrite for VecWriter<'a> {
                fn poll_write(
                    self: Pin<&mut Self>,
                    _cx: &mut std::task::Context<'_>,
                    buf: &[u8],
                ) -> std::task::Poll<std::io::Result<usize>> {
                    self.get_mut().0.extend_from_slice(buf);
                    std::task::Poll::Ready(Ok(buf.len()))
                }
                fn poll_flush(
                    self: Pin<&mut Self>,
                    _cx: &mut std::task::Context<'_>,
                ) -> std::task::Poll<std::io::Result<()>> {
                    std::task::Poll::Ready(Ok(()))
                }
                fn poll_shutdown(
                    self: Pin<&mut Self>,
                    _cx: &mut std::task::Context<'_>,
                ) -> std::task::Poll<std::io::Result<()>> {
                    std::task::Poll::Ready(Ok(()))
                }
            }
            let mut writer = VecWriter(&mut buf);
            self.write_to(Pin::new(&mut writer)).await?;
        }
        Ok(Bytes::from(buf))
    }

    /// Convenience: read as UTF-8 string.
    async fn read_as_string(&mut self) -> Result<String, HttpRequestException> {
        let bytes = self.read_as_bytes().await?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| HttpRequestException::new(format!("body is not valid UTF-8: {e}"), None))
    }

    /// Attempt to clone this body in a way that allows re-sending (e.g. on
    /// redirect). Returns `None` for content types that cannot be cheaply
    /// re-buffered.
    fn try_clone(&self) -> Option<Box<dyn HttpContent>>;
}

// Allow `Box<dyn HttpContent>` itself to be used as `HttpContent`.
#[async_trait]
impl HttpContent for Box<dyn HttpContent> {
    fn headers(&self) -> &HttpContentHeaders {
        (**self).headers()
    }
    fn content_length(&self) -> Option<u64> {
        (**self).content_length()
    }
    async fn write_to(
        &mut self,
        writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException> {
        (**self).write_to(writer).await
    }
    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        (**self).read_as_bytes().await
    }
    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        (**self).try_clone()
    }
}
