//! `StringContent` — a UTF-8 string body.

use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::AsyncWrite;

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;
use crate::media_type::MediaTypeHeaderValue;

/// A UTF-8 string body, equivalent to C#'s `StringContent`.
#[derive(Debug, Clone)]
pub struct StringContent {
    inner: String,
    headers: HttpContentHeaders,
}

impl StringContent {
    /// Create a string body. Defaults to `text/plain; charset=utf-8` unless
    /// `media_type` is supplied.
    pub fn new(inner: impl Into<String>) -> Self {
        Self::with_media_type(inner, "text/plain", Some("utf-8"))
    }

    /// Create a string body with a specific media type and optional charset.
    pub fn with_media_type(
        inner: impl Into<String>,
        media_type: &str,
        charset: Option<&str>,
    ) -> Self {
        let inner = inner.into();
        let mut media = MediaTypeHeaderValue::parse(media_type)
            .expect("valid media type for StringContent::with_media_type");
        if let Some(cs) = charset {
            media = media.with_charset(cs);
        }
        let mut headers = HttpContentHeaders::new();
        headers.set(http::header::CONTENT_TYPE, media.to_header_value());
        Self { inner, headers }
    }

    /// The inner string.
    pub fn value(&self) -> &str {
        &self.inner
    }
}

#[async_trait]
impl HttpContent for StringContent {
    fn headers(&self) -> &HttpContentHeaders {
        &self.headers
    }

    fn content_length(&self) -> Option<u64> {
        Some(self.inner.len() as u64)
    }

    async fn write_to(
        &mut self,
        mut writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException> {
        use tokio::io::AsyncWriteExt;
        writer
            .write_all(self.inner.as_bytes())
            .await
            .map_err(|e| HttpRequestException::new(format!("write failed: {e}"), None))?;
        Ok(())
    }

    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        Ok(Bytes::copy_from_slice(self.inner.as_bytes()))
    }

    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_media_type_is_text_plain_utf8() {
        let c = StringContent::new("hello");
        let mt = c.headers().content_type().unwrap();
        assert_eq!(mt.media_type(), "text");
        assert_eq!(mt.subtype(), "plain");
        assert_eq!(mt.charset(), Some("utf-8"));
    }

    #[tokio::test]
    async fn write_emits_inner_bytes() {
        let mut c = StringContent::new("hello world");
        let bytes = c.read_as_bytes().await.unwrap();
        assert_eq!(&bytes[..], b"hello world");
        assert_eq!(c.content_length(), Some(11));
    }
}
