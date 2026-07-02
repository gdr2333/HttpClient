//! Header collections.
//!
//! Modeled on C#'s `HttpHeaders` / `HttpRequestHeaders` /
//! `HttpResponseHeaders` / `HttpContentHeaders`. Internally this wraps
//! `http::HeaderMap`, which is case-insensitive and supports multi-valued
//! headers.

use std::fmt;

use http::{HeaderMap, HeaderName, HeaderValue};

/// Common header operations shared by request, response, and content
/// headers. Mirrors the public surface of C#'s `HttpHeaders`.
#[derive(Debug, Default, Clone)]
pub struct HttpHeaders {
    inner: HeaderMap,
}

impl HttpHeaders {
    /// Construct an empty header set.
    pub fn new() -> Self {
        Self {
            inner: HeaderMap::new(),
        }
    }

    /// Borrow the underlying `HeaderMap`.
    pub fn as_map(&self) -> &HeaderMap {
        &self.inner
    }

    /// Try to get all values for a header. Returns `None` if the header is
    /// not present. Mirrors C#'s `HttpHeaders.TryGetValues`.
    pub fn try_get_values(&self, name: &HeaderName) -> Option<Vec<&str>> {
        if !self.inner.contains_key(name) {
            return None;
        }
        Some(
            self.inner
                .get_all(name)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .collect(),
        )
    }

    /// Get the first value for a header, or `None`. Mirrors C#'s
    /// `HttpHeaders.GetValues` (which throws if missing — we don't).
    pub fn get(&self, name: &HeaderName) -> Option<&str> {
        self.inner.get(name).and_then(|v| v.to_str().ok())
    }

    /// Add a header value, appending if the header is already present.
    pub fn add(&mut self, name: HeaderName, value: impl AsHeaderValue) {
        self.inner.append(name, value.into_header_value());
    }

    /// Set a header value, replacing any existing values.
    pub fn set(&mut self, name: HeaderName, value: impl AsHeaderValue) {
        self.inner.insert(name, value.into_header_value());
    }

    /// Remove a header and all its values.
    pub fn remove(&mut self, name: HeaderName) {
        self.inner.remove(name);
    }

    /// `true` if the header is present.
    pub fn contains(&self, name: &HeaderName) -> bool {
        self.inner.contains_key(name)
    }

    /// Iterate over `(name, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&HeaderName, &HeaderValue)> {
        self.inner.iter()
    }
}

impl fmt::Display for HttpHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, v) in self.inner.iter() {
            writeln!(f, "{}: {}", k, v.to_str().unwrap_or("<binary>"))?;
        }
        Ok(())
    }
}

/// Request-side headers.
#[derive(Debug, Default, Clone)]
pub struct HttpRequestHeaders(HttpHeaders);

impl HttpRequestHeaders {
    pub fn new() -> Self {
        Self(HttpHeaders::new())
    }
    pub fn inner(&self) -> &HttpHeaders {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut HttpHeaders {
        &mut self.0
    }
}

impl std::ops::Deref for HttpRequestHeaders {
    type Target = HttpHeaders;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for HttpRequestHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Response-side headers.
#[derive(Debug, Default, Clone)]
pub struct HttpResponseHeaders(HttpHeaders);

impl HttpResponseHeaders {
    pub fn new() -> Self {
        Self(HttpHeaders::new())
    }
    pub fn inner(&self) -> &HttpHeaders {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut HttpHeaders {
        &mut self.0
    }
}

impl std::ops::Deref for HttpResponseHeaders {
    type Target = HttpHeaders;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for HttpResponseHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Content-side headers (e.g. `Content-Type`, `Content-Length`).
#[derive(Debug, Default, Clone)]
pub struct HttpContentHeaders(HttpHeaders);

impl HttpContentHeaders {
    pub fn new() -> Self {
        Self(HttpHeaders::new())
    }
    pub fn inner(&self) -> &HttpHeaders {
        &self.0
    }
    pub fn inner_mut(&mut self) -> &mut HttpHeaders {
        &mut self.0
    }

    /// Parsed `Content-Type` if present.
    pub fn content_type(&self) -> Option<crate::media_type::MediaTypeHeaderValue> {
        self.0
            .get(&HeaderName::from_static("content-type"))
            .and_then(|s| s.parse().ok())
    }

    /// `Content-Length` if present.
    pub fn content_length(&self) -> Option<u64> {
        self.0
            .get(&HeaderName::from_static("content-length"))
            .and_then(|s| s.parse().ok())
    }
}

impl std::ops::Deref for HttpContentHeaders {
    type Target = HttpHeaders;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for HttpContentHeaders {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Trait for types that can be coerced into a `HeaderValue`.
pub trait AsHeaderValue {
    fn into_header_value(self) -> HeaderValue;
}

impl AsHeaderValue for &str {
    fn into_header_value(self) -> HeaderValue {
        HeaderValue::from_str(self).expect("valid header value")
    }
}

impl AsHeaderValue for String {
    fn into_header_value(self) -> HeaderValue {
        HeaderValue::from_str(&self).expect("valid header value")
    }
}

impl AsHeaderValue for HeaderValue {
    fn into_header_value(self) -> HeaderValue {
        self
    }
}

impl AsHeaderValue for &HeaderValue {
    fn into_header_value(self) -> HeaderValue {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &'static str) -> HeaderName {
        // HeaderName::from_static requires lowercase ASCII, so lowercase the input.
        let lower: &'static str = Box::leak(s.to_ascii_lowercase().into_boxed_str());
        HeaderName::from_static(lower)
    }

    #[test]
    fn add_and_get_roundtrip() {
        let mut h = HttpHeaders::new();
        h.add(n("x-test"), "1");
        h.add(n("x-test"), "2");
        assert!(h.contains(&n("x-test")));
        assert!(h.contains(&n("X-TEST")));
        let vals = h.try_get_values(&n("X-Test")).unwrap();
        assert_eq!(vals, vec!["1", "2"]);
    }

    #[test]
    fn set_replaces() {
        let mut h = HttpHeaders::new();
        h.add(n("x-test"), "1");
        h.add(n("x-test"), "2");
        h.set(n("x-test"), "9");
        let vals = h.try_get_values(&n("x-test")).unwrap();
        assert_eq!(vals, vec!["9"]);
    }

    #[test]
    fn remove_clears() {
        let mut h = HttpHeaders::new();
        h.add(n("x-test"), "x");
        h.remove(n("x-test"));
        assert!(!h.contains(&n("x-test")));
    }
}
