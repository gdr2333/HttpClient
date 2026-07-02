//! Error types. Modeled on `System.Net.Http.HttpRequestException` and
//! `System.OperationCanceledException` so callers can branch the same way they
//! would in C#.

use std::io;
use thiserror::Error;

/// Concrete error type returned from every fallible call in this crate.
///
/// Variant distinction mirrors the C# exception hierarchy: cancellation
/// surfaces as [`OperationCanceledException`], network/protocol errors as
/// [`HttpRequestException`], and other I/O errors as [`Io`].
#[derive(Debug, Error)]
pub enum HttpRequestError {
    /// A cancellation token was triggered (timeout, user cancel, parent
    /// cancel). Equivalent to C#'s `OperationCanceledException`.
    #[error(transparent)]
    Canceled(#[from] OperationCanceledException),

    /// A network or protocol error. Equivalent to C#'s `HttpRequestException`.
    #[error(transparent)]
    Http(#[from] HttpRequestException),

    /// An underlying I/O error that did not match the other categories.
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<HttpRequestError> for io::Error {
    fn from(err: HttpRequestError) -> Self {
        match err {
            HttpRequestError::Io(io) => io,
            other => io::Error::other(other),
        }
    }
}

/// The closest analogue to `System.Net.Http.HttpRequestException`.
///
/// Constructed by the transport when the server returns an error status code
/// the caller asked us to surface (e.g. via `EnsureSuccessStatusCode`), or
/// when the response is malformed.
#[derive(Debug, Error)]
#[error("{formatted}")]
pub struct HttpRequestException {
    /// Human-readable message, modeled on `HttpRequestException.Message`.
    pub message: String,
    /// The status code from the response, if one was received.
    pub status_code: Option<u16>,
    /// The inner cause, if any.
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
    /// The message + status code, used by `Display`.
    formatted: String,
}

impl HttpRequestException {
    /// Format the message + status code together for `Display`. Pulled out
    /// so the test can assert against it.
    fn make_formatted(message: &str, status_code: Option<u16>) -> String {
        match status_code {
            Some(code) => format!("HTTP {code}: {message}"),
            None => message.to_string(),
        }
    }
}

impl HttpRequestException {
    /// Construct a new exception with a message and optional status code.
    pub fn new(message: impl Into<String>, status_code: Option<u16>) -> Self {
        let message = message.into();
        let formatted = Self::make_formatted(&message, status_code);
        Self {
            message,
            status_code,
            source: None,
            formatted,
        }
    }

    /// Attach an underlying cause.
    #[must_use]
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }
}

/// The closest analogue to `System.OperationCanceledException`.
#[derive(Debug, Error)]
#[error("operation canceled")]
pub struct OperationCanceledException {
    /// The cancellation message (e.g. `"timeout"` or the caller's own text).
    pub message: String,
}

impl OperationCanceledException {
    /// Construct a new cancellation exception.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Default for OperationCanceledException {
    fn default() -> Self {
        Self {
            message: "operation canceled".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_includes_status_code() {
        let err = HttpRequestException::new("bad request", Some(400));
        assert!(err.to_string().contains("400"));
        assert!(err.to_string().contains("bad request"));
    }

    #[test]
    fn operation_canceled_default_message() {
        let err = OperationCanceledException::default();
        assert_eq!(err.message, "operation canceled");
    }
}
