//! `HttpMethod` — the HTTP verb (GET, POST, PUT, etc.).

use std::fmt;
use std::str::FromStr;

/// HTTP method, modeled on C#'s `System.Net.Http.HttpMethod`.
///
/// The standard verbs are first-class variants; custom verbs are stored as
/// `Custom(String)`. The standard ones normalize to their canonical
/// upper-case form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    /// `GET`
    Get,
    /// `POST`
    Post,
    /// `PUT`
    Put,
    /// `DELETE`
    Delete,
    /// `HEAD`
    Head,
    /// `OPTIONS`
    Options,
    /// `PATCH`
    Patch,
    /// `TRACE`
    Trace,
    /// `CONNECT`
    Connect,
    /// A non-standard method, e.g. `"PROPFIND"`. Stored verbatim.
    Custom(String),
}

impl HttpMethod {
    /// Construct a method from any string, normalizing the standard verbs.
    pub fn from_string(s: impl Into<String>) -> Self {
        let s = s.into();
        match s.to_ascii_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            "PATCH" => Self::Patch,
            "TRACE" => Self::Trace,
            "CONNECT" => Self::Connect,
            other => Self::Custom(other.to_string()),
        }
    }

    /// Return the wire representation (e.g. `"GET"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Patch => "PATCH",
            Self::Trace => "TRACE",
            Self::Connect => "CONNECT",
            Self::Custom(s) => s,
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_string(s))
    }
}

impl From<&str> for HttpMethod {
    fn from(s: &str) -> Self {
        Self::from_string(s)
    }
}

impl From<String> for HttpMethod {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_verbs_canonicalize() {
        assert_eq!(HttpMethod::from("get"), HttpMethod::Get);
        assert_eq!(HttpMethod::from("Post"), HttpMethod::Post);
        assert_eq!(HttpMethod::from("PATCH"), HttpMethod::Patch);
    }

    #[test]
    fn custom_verb_preserved() {
        let m = HttpMethod::from("PROPFIND");
        assert_eq!(m, HttpMethod::Custom("PROPFIND".to_string()));
        assert_eq!(m.as_str(), "PROPFIND");
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Custom("PURGE".into()).to_string(), "PURGE");
    }
}
