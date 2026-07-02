//! `SocketsHttpHandler` — the default `HttpMessageHandler` that talks
//! HTTP/1.1 over tokio TCP, with TLS via rustls. Equivalent to C#'s
//! `SocketsHttpHandler`.

use async_trait::async_trait;

use crate::cancellation::CancellationToken;
use crate::error::HttpRequestError;
use crate::handlers::HttpMessageHandler;
use crate::request::HttpRequestMessage;
use crate::response::HttpResponseMessage;
use crate::transport::pool::ConnectionPool;

/// Default transport for `HttpClient`. Mirrors C#'s `SocketsHttpHandler`.
#[derive(Debug, Clone)]
pub struct SocketsHttpHandler {
    /// The connection pool. Cloned cheaply (internally `Arc`-shared).
    pool: ConnectionPool,
    /// Maximum number of redirects to follow. `0` disables auto-redirect.
    pub max_redirections: u8,
    /// If `true`, follow 3xx responses up to `max_redirections` times.
    pub allow_auto_redirect: bool,
}

impl SocketsHttpHandler {
    /// Create a handler with a fresh in-process connection pool and default
    /// settings.
    pub fn new() -> Self {
        Self {
            pool: ConnectionPool::new(),
            max_redirections: 5,
            allow_auto_redirect: true,
        }
    }

    /// Borrow the connection pool.
    pub fn pool(&self) -> &ConnectionPool {
        &self.pool
    }
}

impl Default for SocketsHttpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpMessageHandler for SocketsHttpHandler {
    async fn send_async(
        &self,
        request: HttpRequestMessage,
        cancellation_token: CancellationToken,
    ) -> Result<HttpResponseMessage, HttpRequestError> {
        super::execute::execute_send(self, request, cancellation_token).await
    }
}
