//! `HttpVersion` and `HttpVersionPolicy` — wire protocol version and the
//! policy for selecting it.

/// HTTP protocol version. Re-exported from the `http` crate so the rest of
/// this crate does not have to depend on it directly.
pub use http::Version as HttpVersion;

/// Constants for the common versions. Re-exported from `http`.
pub use http::Version;

/// C#'s `HttpVersionPolicy` — how strictly to honor `RequestVersion`.
///
/// Even though this crate only supports HTTP/1.1, the policy is preserved for
/// API compatibility: callers can ask for `RequestVersionOrLower` so that if a
/// later version adds HTTP/2/3, the caller's policy continues to work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HttpVersionPolicy {
    /// Use the request's `Version` field exactly.
    RequestVersionExact,
    /// Use the request's `Version`, or an older version if the server doesn't
    /// support it. Equivalent to C#'s default.
    #[default]
    RequestVersionOrLower,
    /// Use the request's `Version`, or a newer version if the server supports
    /// it.
    RequestVersionOrHigher,
}
