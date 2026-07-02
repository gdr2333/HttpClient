//! `ByteArrayContent` — a fixed byte body.

use std::pin::Pin;

use ::bytes::Bytes;
use async_trait::async_trait;
use tokio::io::AsyncWrite;

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;

/// A `Vec<u8>` / `Bytes` body, equivalent to C#'s `ByteArrayContent`.
#[derive(Debug, Clone)]
pub struct ByteArrayContent {
    inner: Bytes,
    headers: HttpContentHeaders,
}

impl ByteArrayContent {
    /// Create a byte body. Defaults to `application/octet-stream`.
    pub fn new(bytes: impl Into<Bytes>) -> Self {
        Self::with_media_type(bytes, "application/octet-stream")
    }

    /// Create a byte body with a specific media type.
    pub fn with_media_type(bytes: impl Into<Bytes>, media_type: &str) -> Self {
        let bytes = bytes.into();
        let mut headers = HttpContentHeaders::new();
        let mt: http::HeaderValue = media_type.to_string().parse().expect("valid media type");
        headers.set(http::header::CONTENT_TYPE, mt);
        Self {
            inner: bytes,
            headers,
        }
    }
}

#[async_trait]
impl HttpContent for ByteArrayContent {
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
            .write_all(&self.inner)
            .await
            .map_err(|e| HttpRequestException::new(format!("write failed: {e}"), None))?;
        Ok(())
    }

    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        Ok(self.inner.clone())
    }

    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_emits_bytes_verbatim() {
        let mut c = ByteArrayContent::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let bytes = c.read_as_bytes().await.unwrap();
        assert_eq!(&bytes[..], &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(c.content_length(), Some(4));
    }
}
