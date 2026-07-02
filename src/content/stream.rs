//! `StreamContent` — a body read lazily from an `AsyncRead`.

use std::pin::Pin;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;

/// A body whose bytes are read lazily from an `AsyncRead` source, equivalent
/// to C#'s `StreamContent`.
pub struct StreamContent {
    /// Stored as `Box<dyn AsyncRead + Send + Unpin>` so we can read it back
    /// on demand. The original source is consumed.
    source: Option<Pin<Box<dyn AsyncRead + Send + Unpin>>>,
    /// Cached bytes after `read_as_bytes`. If present, subsequent calls
    /// return this.
    cached: Option<Bytes>,
    headers: HttpContentHeaders,
}

impl std::fmt::Debug for StreamContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamContent")
            .field("headers", &self.headers)
            .field("cached", &self.cached.as_ref().map(|b| b.len()))
            .finish()
    }
}

impl StreamContent {
    /// Create a stream body. The default `Content-Type` is
    /// `application/octet-stream`; set it explicitly via
    /// [`StreamContent::with_media_type`].
    pub fn new<R>(source: R) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        Self::with_media_type(source, "application/octet-stream")
    }

    /// Create a stream body with an explicit media type.
    pub fn with_media_type<R>(source: R, media_type: &str) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let mut headers = HttpContentHeaders::new();
        let mt: http::HeaderValue = format!("{media_type}").parse().expect("valid media type");
        headers.set(http::header::CONTENT_TYPE, mt);
        Self {
            source: Some(Box::pin(source)),
            cached: None,
            headers,
        }
    }
}

#[async_trait]
impl HttpContent for StreamContent {
    fn headers(&self) -> &HttpContentHeaders {
        &self.headers
    }

    fn content_length(&self) -> Option<u64> {
        // Streaming bodies don't know the length up front.
        None
    }

    async fn write_to(
        &mut self,
        mut writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException> {
        use tokio::io::AsyncWriteExt;
        // Buffer the source lazily (consuming it), then write the cache.
        let buf = self.read_as_bytes().await?;
        writer
            .write_all(&buf)
            .await
            .map_err(|e| HttpRequestException::new(format!("write failed: {e}"), None))?;
        Ok(())
    }

    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        if let Some(buf) = &self.cached {
            return Ok(buf.clone());
        }
        if let Some(mut source) = self.source.take() {
            let mut buf = BytesMut::new();
            let mut tmp = [0u8; 8192];
            loop {
                use tokio::io::AsyncReadExt;
                let n = match source.read(&mut tmp).await {
                    Ok(n) => n,
                    Err(e) => {
                        return Err(HttpRequestException::new(
                            format!("stream read: {e}"),
                            None,
                        ));
                    }
                };
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let bytes = buf.freeze();
            self.cached = Some(bytes.clone());
            Ok(bytes)
        } else {
            Err(HttpRequestException::new(
                "StreamContent source has been consumed",
                None,
            ))
        }
    }

    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        None
    }
}
