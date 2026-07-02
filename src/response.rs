//! `HttpResponseMessage` — a single response, equivalent to C#'s
//! `System.Net.Http.HttpResponseMessage`.

use bytes::Bytes;
use http::StatusCode;

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpResponseHeaders;
use crate::request::HttpRequestMessage;
use crate::version::HttpVersion;

/// An HTTP response.
///
/// Mirrors the C# shape: a status code, a version, a header collection, and
/// a body. The body is always non-null in C# (it defaults to an empty
/// `StreamContent`); we follow the same convention.
pub struct HttpResponseMessage {
    status_code: StatusCode,
    version: HttpVersion,
    headers: HttpResponseHeaders,
    content: Box<dyn HttpContent>,
    request_message: Option<HttpRequestMessage>,
}

impl std::fmt::Debug for HttpResponseMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpResponseMessage")
            .field("status_code", &self.status_code)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .field("content", &self.content.content_length())
            .field("request_message", &self.request_message)
            .finish()
    }
}

impl HttpResponseMessage {
    /// Construct a new response. The body defaults to an empty
    /// `ByteArrayContent`.
    pub fn new(status_code: StatusCode) -> Self {
        Self {
            status_code,
            version: HttpVersion::HTTP_11,
            headers: HttpResponseHeaders::new(),
            content: Box::new(crate::content::ByteArrayContent::new(Bytes::new())),
            request_message: None,
        }
    }

    /// The status code.
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// Set the status code.
    pub fn set_status_code(&mut self, code: StatusCode) {
        self.status_code = code;
    }

    /// `true` if the status code is in the 2xx range.
    pub fn is_success_status_code(&self) -> bool {
        self.status_code.is_success()
    }

    /// Throw `HttpRequestException` if the status is not 2xx. Returns
    /// `self` on success.
    pub fn ensure_success_status_code(self) -> Result<Self, HttpRequestException> {
        if self.is_success_status_code() {
            Ok(self)
        } else {
            let code = self.status_code.as_u16();
            let phrase = self
                .status_code
                .canonical_reason()
                .unwrap_or("")
                .to_string();
            Err(HttpRequestException::new(
                format!("HTTP {code} {phrase}"),
                Some(code),
            ))
        }
    }

    /// The HTTP version.
    pub fn version(&self) -> HttpVersion {
        self.version
    }

    /// Set the HTTP version.
    pub fn set_version(&mut self, version: HttpVersion) {
        self.version = version;
    }

    /// The response headers.
    pub fn headers(&self) -> &HttpResponseHeaders {
        &self.headers
    }

    /// Mutable access to the response headers.
    pub fn headers_mut(&mut self) -> &mut HttpResponseHeaders {
        &mut self.headers
    }

    /// The response body.
    pub fn content(&self) -> &dyn HttpContent {
        &*self.content
    }

    /// Mutable access to the response body. Used by callers that want to
    /// read the body via `read_as_bytes` / `read_as_string`.
    pub fn content_mut(&mut self) -> &mut dyn HttpContent {
        &mut *self.content
    }

    /// Set the response body.
    pub fn set_content(&mut self, content: Box<dyn HttpContent>) {
        self.content = content;
    }

    /// Back-pointer to the originating request, if any.
    pub fn request_message(&self) -> Option<&HttpRequestMessage> {
        self.request_message.as_ref()
    }

    /// Set the back-pointer.
    pub fn set_request_message(&mut self, request: Option<HttpRequestMessage>) {
        self.request_message = request;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_response_is_unsuccessful_for_4xx() {
        let r = HttpResponseMessage::new(StatusCode::NOT_FOUND);
        assert_eq!(r.status_code(), StatusCode::NOT_FOUND);
        assert!(!r.is_success_status_code());
        assert!(r.ensure_success_status_code().is_err());
    }

    #[test]
    fn new_response_is_successful_for_2xx() {
        let r = HttpResponseMessage::new(StatusCode::OK);
        assert!(r.is_success_status_code());
        assert!(r.ensure_success_status_code().is_ok());
    }
}
