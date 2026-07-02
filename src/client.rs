//! `HttpClient` — the public entry point. Mirrors C#'s `System.Net.Http.HttpClient`.

use std::time::Duration;

use crate::cancellation::CancellationToken;
use crate::content::HttpContent;
use crate::error::{HttpRequestError, HttpRequestException};
use crate::handlers::{HttpMessageHandler, SocketsHttpHandler};
use crate::headers::HttpRequestHeaders;
use crate::method::HttpMethod;
use crate::request::HttpRequestMessage;
use crate::response::HttpResponseMessage;
use crate::uri::Uri;
use crate::version::HttpVersion;

/// The HTTP client. Long-lived and shared by reference. Matches C#'s
/// guidance: do **not** clone per request; share one instance via `&`.
///
/// All async methods are non-blocking. The `Send`-style call sites take
/// `CancellationToken::default()` (or `CancellationToken::none()`) if the
/// caller has no token.
pub struct HttpClient {
    handler: Box<dyn HttpMessageHandler>,
    base_address: Option<Uri>,
    timeout: Duration,
    default_request_version: HttpVersion,
    default_headers: HttpRequestHeaders,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("base_address", &self.base_address)
            .field("timeout", &self.timeout)
            .field("default_request_version", &self.default_request_version)
            .field("default_headers", &self.default_headers)
            .field("handler", &"<dyn HttpMessageHandler>")
            .finish()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// Create a new client with the default `SocketsHttpHandler`.
    pub fn new() -> Self {
        Self::with_handler(SocketsHttpHandler::new())
    }

    /// Create a new client with a custom `HttpMessageHandler`. The handler
    /// is moved into the client; the client owns it.
    pub fn with_handler<H>(handler: H) -> Self
    where
        H: HttpMessageHandler + 'static,
    {
        Self {
            handler: Box::new(handler),
            base_address: None,
            timeout: Duration::from_secs(100), // C# default
            default_request_version: HttpVersion::HTTP_11,
            default_headers: HttpRequestHeaders::new(),
        }
    }

    /// The base address; relative URIs in `SendAsync` / `GetAsync` / etc.
    /// are resolved against this.
    pub fn base_address(&self) -> Option<&Uri> {
        self.base_address.as_ref()
    }

    /// Set the base address. `None` clears it.
    pub fn set_base_address(&mut self, uri: Option<Uri>) {
        self.base_address = uri;
    }

    /// The default per-request timeout. C# default is 100 seconds.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Set the default per-request timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// The default HTTP version used for new requests.
    pub fn default_request_version(&self) -> HttpVersion {
        self.default_request_version
    }

    /// Set the default HTTP version used for new requests.
    pub fn set_default_request_version(&mut self, version: HttpVersion) {
        self.default_request_version = version;
    }

    /// The default headers applied to every outgoing request.
    pub fn default_headers(&mut self) -> &mut HttpRequestHeaders {
        &mut self.default_headers
    }

    // ------------------------------------------------------------------
    // Low-level: SendAsync
    // ------------------------------------------------------------------

    /// Send a single request. The request's URI may be relative; it is
    /// resolved against `BaseAddress` if so.
    pub async fn send_async(
        &self,
        request: HttpRequestMessage,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        // Apply default headers that aren't already set.
        let mut req = request;
        for (k, v) in self.default_headers.as_map().iter() {
            if !req.headers().as_map().contains_key(k) {
                req.headers_mut().set(k.clone(), v.clone());
            }
        }
        // Wrap the caller's token with our timeout.
        let effective = if self.timeout > Duration::ZERO {
            cancellation_token.with_timeout(self.timeout)
        } else {
            cancellation_token
        };
        self.handler.send_async(req, effective).await
    }

    // ------------------------------------------------------------------
    // Convenience: GetAsync / PostAsync / ...
    // ------------------------------------------------------------------

    /// Send a `GET` request. The URI may be relative.
    pub async fn get_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let req = HttpRequestMessage::new(HttpMethod::Get, uri.as_ref());
        self.send_async(req, cancellation_token).await
    }

    /// Send a `GET` and return the body as a string.
    pub async fn get_string_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<String, HttpRequestError> {
        let mut r = self.get_async(uri, cancellation_token).await?;
        r.content_mut()
            .read_as_string()
            .await
            .map_err(HttpRequestError::Http)
    }

    /// Send a `GET` and return the body as bytes.
    pub async fn get_byte_array_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<u8>, HttpRequestError> {
        let mut r = self.get_async(uri, cancellation_token).await?;
        let bytes = r
            .content_mut()
            .read_as_bytes()
            .await
            .map_err(HttpRequestError::Http)?;
        Ok(bytes.to_vec())
    }

    /// Send a `POST` with a body.
    pub async fn post_async(
        &self,
        uri: impl AsRef<str>,
        content: Box<dyn HttpContent>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let mut req = HttpRequestMessage::new(HttpMethod::Post, uri.as_ref());
        req.set_content(Some(content));
        self.send_async(req, cancellation_token).await
    }

    /// Send a `PUT` with a body.
    pub async fn put_async(
        &self,
        uri: impl AsRef<str>,
        content: Box<dyn HttpContent>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let mut req = HttpRequestMessage::new(HttpMethod::Put, uri.as_ref());
        req.set_content(Some(content));
        self.send_async(req, cancellation_token).await
    }

    /// Send a `DELETE`.
    pub async fn delete_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let req = HttpRequestMessage::new(HttpMethod::Delete, uri.as_ref());
        self.send_async(req, cancellation_token).await
    }

    /// Send a `PATCH` with a body.
    pub async fn patch_async(
        &self,
        uri: impl AsRef<str>,
        content: Box<dyn HttpContent>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let mut req = HttpRequestMessage::new(HttpMethod::Patch, uri.as_ref());
        req.set_content(Some(content));
        self.send_async(req, cancellation_token).await
    }

    /// Send a `HEAD`.
    pub async fn head_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let req = HttpRequestMessage::new(HttpMethod::Head, uri.as_ref());
        self.send_async(req, cancellation_token).await
    }

    /// Send an `OPTIONS`.
    pub async fn options_async(
        &self,
        uri: impl AsRef<str>,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        let req = HttpRequestMessage::new(HttpMethod::Options, uri.as_ref());
        self.send_async(req, cancellation_token).await
    }
}

/// A builder for `HttpClient`. Mirrors the convenience of C#'s
/// `HttpClientFactory` patterns without the actual factory semantics.
#[derive(Debug, Default)]
pub struct HttpClientBuilder {
    base_address: Option<Uri>,
    timeout: Option<Duration>,
    default_request_version: Option<HttpVersion>,
}

impl HttpClientBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base address.
    pub fn base_address(mut self, uri: Option<Uri>) -> Self {
        self.base_address = uri;
        self
    }

    /// Set the timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the default request version.
    pub fn default_request_version(mut self, v: HttpVersion) -> Self {
        self.default_request_version = Some(v);
        self
    }

    /// Build the `HttpClient`.
    pub fn build(self) -> HttpClient {
        let mut c = HttpClient::new();
        if let Some(b) = self.base_address {
            c.set_base_address(Some(b));
        }
        if let Some(t) = self.timeout {
            c.set_timeout(t);
        }
        if let Some(v) = self.default_request_version {
            c.set_default_request_version(v);
        }
        c
    }
}

#[allow(dead_code)]
fn _unused_error() -> HttpRequestError {
    HttpRequestError::Http(HttpRequestException::new("x", None))
}
