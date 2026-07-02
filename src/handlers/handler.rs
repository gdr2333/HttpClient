//! `HttpMessageHandler` — the trait that all transport implementations
//! implement. Mirrors C#'s `System.Net.Http.HttpMessageHandler`.

use async_trait::async_trait;

use crate::cancellation::CancellationToken;
use crate::error::HttpRequestError;
use crate::request::HttpRequestMessage;
use crate::response::HttpResponseMessage;

/// A transport. Send a request, get a response (or an error). The handler
/// owns the wire details: TCP, TLS, HTTP/1.1 framing, redirects, etc.
#[async_trait]
pub trait HttpMessageHandler: Send + Sync {
    /// Send the request. If the token is cancelled before or during the
    /// send, the returned future resolves to
    /// `HttpRequestError::Canceled(OperationCanceledException)`.
    async fn send_async(
        &self,
        request: HttpRequestMessage,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError>;
}
