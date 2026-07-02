//! `Uri` — a thin wrapper around `url::Url` that adds the `BaseAddress`
//! relative-resolution semantics needed by `HttpClient`.
//!
//! This is deliberately a value type with `Clone` so it can be passed by
//! value and stored in headers.

use std::fmt;
use std::str::FromStr;

use url::Url;

/// HTTP URI. Wraps `url::Url`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uri(Url);

impl Uri {
    /// Parse a URI from a string. Returns an error if the input is not a
    /// valid URI.
    pub fn parse(s: &str) -> Result<Self, UriError> {
        Url::parse(s)
            .map(Self)
            .map_err(|e| UriError(format!("invalid uri: {e}")))
    }

    /// Construct from a parsed `url::Url`.
    pub fn from_url(url: Url) -> Self {
        Self(url)
    }

    /// Borrow the inner `url::Url`.
    pub fn as_url(&self) -> &Url {
        &self.0
    }

    /// Resolve `self` against a base, matching the semantics of C#'s
    /// `Uri(base, relative)` constructor. If `self` is already absolute, it
    /// is returned unchanged.
    pub fn resolve_against(base: &Uri, relative: &str) -> Result<Self, UriError> {
        let url = base
            .0
            .join(relative)
            .map_err(|e| UriError(format!("relative resolution failed: {e}")))?;
        Ok(Self(url))
    }

    /// The scheme (`http`, `https`, ...).
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    /// The host portion of the URI.
    pub fn host(&self) -> Option<&str> {
        self.0.host_str()
    }

    /// The port, if explicitly set.
    pub fn port(&self) -> Option<u16> {
        self.0.port()
    }

    /// The path portion of the URI.
    pub fn path(&self) -> &str {
        self.0.path()
    }

    /// The query string, if any.
    pub fn query(&self) -> Option<&str> {
        self.0.query()
    }

    /// `true` if the scheme is `https`.
    pub fn is_https(&self) -> bool {
        self.0.scheme() == "https"
    }

    /// `true` if the scheme is `http`.
    pub fn is_http(&self) -> bool {
        self.0.scheme() == "http"
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Uri {
    type Err = UriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<Url> for Uri {
    fn from(u: Url) -> Self {
        Self(u)
    }
}

/// Error returned when a URI cannot be parsed or resolved.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct UriError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_absolute_uri() {
        let u = Uri::parse("https://example.com/path?a=1").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
    }

    #[test]
    fn resolve_relative_against_base() {
        let base = Uri::parse("https://example.com/api/").unwrap();
        let resolved = Uri::resolve_against(&base, "users/42").unwrap();
        assert_eq!(resolved.path(), "/api/users/42");
    }

    #[test]
    fn is_https_distinguishes_scheme() {
        assert!(Uri::parse("https://x").unwrap().is_https());
        assert!(!Uri::parse("http://x").unwrap().is_https());
    }
}
