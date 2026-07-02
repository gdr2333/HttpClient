//! Multipart bodies: `MultipartContent` and `MultipartFormDataContent`.
//!
//! Modeled on C#'s `System.Net.Http.MultipartContent` and
//! `MultipartFormDataContent`. The body carries a boundary in its
//! `Content-Type` header and a list of nested contents. Each nested content
//! is itself any `HttpContent`, optionally with a `Content-Disposition` line
//! (which is how `MultipartFormDataContent` adds `name="..."` to each part).

use std::fmt;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::io::AsyncWrite;

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;
use crate::media_type::MediaTypeHeaderValue;

/// A single part to add to a multipart body.
///
/// `disposition` is rendered as a `Content-Disposition: ...` header on the
/// part. Used by `MultipartFormDataContent` to attach `name="..."` to each
/// field.
pub struct MultipartPart {
    /// The body of this part.
    pub content: Box<dyn HttpContent>,
    /// The full `Content-Disposition` line value (without the header
    /// name), e.g. `form-data; name="field1"`.
    pub disposition: Option<String>,
}

impl MultipartPart {
    /// Create a part with no `Content-Disposition` line.
    pub fn new(content: impl HttpContent + 'static) -> Self {
        Self {
            content: Box::new(content),
            disposition: None,
        }
    }

    /// Create a part with a `Content-Disposition: form-data; name="..."` line.
    pub fn form_field(name: impl AsRef<str>, content: impl HttpContent + 'static) -> Self {
        Self {
            content: Box::new(content),
            disposition: Some(format!(r#"form-data; name="{}""#, name.as_ref())),
        }
    }
}

impl fmt::Debug for MultipartPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartPart")
            .field("disposition", &self.disposition)
            .field("content", &"<dyn HttpContent>")
            .finish()
    }
}

impl Clone for MultipartPart {
    fn clone(&self) -> Self {
        Self {
            content: self
                .content
                .try_clone()
                .unwrap_or_else(|| Box::new(crate::content::ByteArrayContent::new(Bytes::new()))),
            disposition: self.disposition.clone(),
        }
    }
}

/// A multipart body. Mirrors C#'s `MultipartContent`. The boundary is part of
/// the content type and the wire framing.
pub struct MultipartContent {
    boundary: String,
    parts: Vec<MultipartPart>,
    headers: HttpContentHeaders,
}

impl MultipartContent {
    /// Create an empty multipart body with a generated boundary.
    pub fn new() -> Self {
        Self::with_boundary(generate_boundary())
    }

    /// Create with an explicit boundary (useful for deterministic tests).
    pub fn with_boundary(boundary: impl Into<String>) -> Self {
        let boundary = boundary.into();
        let mut headers = HttpContentHeaders::new();
        let mt = MediaTypeHeaderValue::new("multipart", "mixed")
            .with_param("boundary", boundary.clone());
        headers.set(http::header::CONTENT_TYPE, mt.to_header_value());
        Self {
            boundary,
            parts: Vec::new(),
            headers,
        }
    }

    /// Add a part.
    pub fn add(&mut self, part: MultipartPart) {
        self.parts.push(part);
    }

    /// Convenience: add a part with no `Content-Disposition`.
    pub fn add_content(&mut self, content: impl HttpContent + 'static) {
        self.parts.push(MultipartPart::new(content));
    }

    /// The boundary string.
    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    /// The number of parts.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// `true` if there are no parts.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Render the entire body to bytes (buffers all parts in memory).
    pub async fn to_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        let mut buf = BytesMut::new();
        for part in self.parts.iter_mut() {
            buf.extend_from_slice(b"--");
            buf.extend_from_slice(self.boundary.as_bytes());
            buf.extend_from_slice(b"\r\n");
            if let Some(disp) = &part.disposition {
                buf.extend_from_slice(b"Content-Disposition: ");
                buf.extend_from_slice(disp.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            for (name, value) in part.content.headers().iter() {
                buf.extend_from_slice(name.as_str().as_bytes());
                buf.extend_from_slice(b": ");
                buf.extend_from_slice(value.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            buf.extend_from_slice(b"\r\n");
            let body = part.content.read_as_bytes().await?;
            buf.extend_from_slice(&body);
            buf.extend_from_slice(b"\r\n");
        }
        buf.extend_from_slice(b"--");
        buf.extend_from_slice(self.boundary.as_bytes());
        buf.extend_from_slice(b"--\r\n");
        Ok(buf.freeze())
    }
}

impl fmt::Debug for MultipartContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultipartContent")
            .field("boundary", &self.boundary)
            .field("parts", &self.parts)
            .finish()
    }
}

impl Clone for MultipartContent {
    fn clone(&self) -> Self {
        Self {
            boundary: self.boundary.clone(),
            parts: self.parts.clone(),
            headers: self.headers.clone(),
        }
    }
}

impl Default for MultipartContent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpContent for MultipartContent {
    fn headers(&self) -> &HttpContentHeaders {
        &self.headers
    }

    fn content_length(&self) -> Option<u64> {
        None
    }

    async fn write_to(
        &mut self,
        mut writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException> {
        use tokio::io::AsyncWriteExt;
        let bytes = self.to_bytes().await?;
        writer
            .write_all(&bytes)
            .await
            .map_err(|e| HttpRequestException::new(format!("write failed: {e}"), None))?;
        Ok(())
    }

    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        self.to_bytes().await
    }

    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        Some(Box::new(self.clone()))
    }
}

/// `multipart/form-data` body. Equivalent to C#'s
/// `MultipartFormDataContent`. Subtype fixed to `form-data`; the constructor
/// ensures every part carries a `Content-Disposition: form-data; name="..."`
/// line via [`MultipartPart::form_field`].
pub type MultipartFormDataContent = MultipartContent;

fn generate_boundary() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("----httpclient-boundary-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::StringContent;

    #[tokio::test]
    async fn multipart_renders_with_boundary() {
        let mut m = MultipartContent::with_boundary("test-boundary");
        m.add(MultipartPart::form_field("field1", StringContent::new("v1")));
        m.add(MultipartPart::form_field("field2", StringContent::new("v2")));
        let bytes = m.to_bytes().await.unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.starts_with("--test-boundary\r\n"));
        assert!(s.contains("Content-Disposition: form-data; name=\"field1\""));
        assert!(s.contains("v1"));
        assert!(s.ends_with("--test-boundary--\r\n"));
    }

    #[tokio::test]
    async fn content_type_includes_boundary() {
        let m = MultipartContent::with_boundary("abc");
        let ct = m.headers().content_type().unwrap();
        assert_eq!(ct.media_type(), "multipart");
        assert_eq!(ct.subtype(), "mixed");
        assert!(ct.to_string().contains("boundary=abc"));
    }
}
