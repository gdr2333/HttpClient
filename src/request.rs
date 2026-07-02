//! `HttpRequestMessage` — a single request, equivalent to C#'s
//! `System.Net.Http.HttpRequestMessage`.

use std::fmt;

use crate::content::HttpContent;
use crate::headers::HttpRequestHeaders;
use crate::method::HttpMethod;
use crate::uri::Uri;
use crate::version::{HttpVersion, HttpVersionPolicy};

/// An HTTP request.
///
/// Mirrors the C# shape: a method, an optional URI (when using
/// `HttpClient.BaseAddress`), a version, a version policy, a header
/// collection, and an optional body.
pub struct HttpRequestMessage {
    method: HttpMethod,
    request_uri: Option<Uri>,
    version: HttpVersion,
    version_policy: HttpVersionPolicy,
    headers: HttpRequestHeaders,
    content: Option<Box<dyn HttpContent>>,
}

impl fmt::Debug for HttpRequestMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequestMessage")
            .field("method", &self.method)
            .field("request_uri", &self.request_uri)
            .field("version", &self.version)
            .field("version_policy", &self.version_policy)
            .field("headers", &self.headers)
            .field("content", &self.content.as_ref().map(|c| c.content_length()))
            .finish()
    }
}

impl Clone for HttpRequestMessage {
    fn clone(&self) -> Self {
        Self {
            method: self.method.clone(),
            request_uri: self.request_uri.clone(),
            version: self.version,
            version_policy: self.version_policy,
            headers: self.headers.clone(),
            content: self.content.as_ref().and_then(|c| c.try_clone()),
        }
    }
}

impl HttpRequestMessage {
    /// Construct a `GET` request for a relative or absolute URI string.
    pub fn get(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Get, uri)
    }

    /// Construct a `POST` request for a relative or absolute URI string.
    pub fn post(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Post, uri)
    }

    /// Construct a `PUT` request for a relative or absolute URI string.
    pub fn put(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Put, uri)
    }

    /// Construct a `DELETE` request for a relative or absolute URI string.
    pub fn delete(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Delete, uri)
    }

    /// Construct a `HEAD` request for a relative or absolute URI string.
    pub fn head(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Head, uri)
    }

    /// Construct a `PATCH` request for a relative or absolute URI string.
    pub fn patch(uri: impl AsRef<str>) -> Self {
        Self::new(HttpMethod::Patch, uri)
    }

    /// Construct a request for any method and URI string. The URI is
    /// parsed lazily by the client; an empty string means "use
    /// `HttpClient.BaseAddress`".
    pub fn new(method: HttpMethod, uri: impl AsRef<str>) -> Self {
        let request_uri = if uri.as_ref().is_empty() {
            None
        } else {
            Some(Uri::parse(uri.as_ref()).expect("valid request URI"))
        };
        Self {
            method,
            request_uri,
            version: HttpVersion::HTTP_11,
            version_policy: HttpVersionPolicy::default(),
            headers: HttpRequestHeaders::new(),
            content: None,
        }
    }

    /// The HTTP method.
    pub fn method(&self) -> &HttpMethod {
        &self.method
    }

    /// Set the HTTP method.
    pub fn set_method(&mut self, method: HttpMethod) {
        self.method = method;
    }

    /// The request URI, if one was set on this message. `None` means "use
    /// `HttpClient.BaseAddress`".
    pub fn request_uri(&self) -> Option<&Uri> {
        self.request_uri.as_ref()
    }

    /// Set the request URI.
    pub fn set_request_uri(&mut self, uri: Option<Uri>) {
        self.request_uri = uri;
    }

    /// The HTTP version.
    pub fn version(&self) -> HttpVersion {
        self.version
    }

    /// Set the HTTP version.
    pub fn set_version(&mut self, version: HttpVersion) {
        self.version = version;
    }

    /// The version-negotiation policy.
    pub fn version_policy(&self) -> HttpVersionPolicy {
        self.version_policy
    }

    /// Set the version-negotiation policy.
    pub fn set_version_policy(&mut self, policy: HttpVersionPolicy) {
        self.version_policy = policy;
    }

    /// The request headers.
    pub fn headers(&self) -> &HttpRequestHeaders {
        &self.headers
    }

    /// Mutable access to the request headers.
    pub fn headers_mut(&mut self) -> &mut HttpRequestHeaders {
        &mut self.headers
    }

    /// The body, if any.
    pub fn content(&self) -> Option<&(dyn HttpContent + 'static)> {
        self.content.as_deref()
    }

    /// Mutable access to the body, if any. Used by the transport to write
    /// the body to the wire.
    pub fn content_mut(&mut self) -> Option<&mut (dyn HttpContent + 'static)> {
        self.content.as_deref_mut()
    }

    /// Set the body. Pass `None` to clear the body.
    pub fn set_content(&mut self, content: Option<Box<dyn HttpContent>>) {
        self.content = content;
    }
}

impl Default for HttpRequestMessage {
    fn default() -> Self {
        Self::new(HttpMethod::Get, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_methods() {
        let r = HttpRequestMessage::post("https://example.com/a");
        assert_eq!(*r.method(), HttpMethod::Post);
        assert_eq!(r.version(), HttpVersion::HTTP_11);
    }
}
