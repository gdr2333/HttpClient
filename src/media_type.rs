//! `MediaTypeHeaderValue` — parse and re-serialize a `Content-Type`-style
//! media type. Modeled on C#'s `System.Net.Http.Headers.MediaTypeHeaderValue`.

use std::fmt;
use std::str::FromStr;

use http::HeaderValue;

/// Parsed media type, e.g. `text/html; charset=utf-8`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTypeHeaderValue {
    type_: String,
    subtype: String,
    charset: Option<String>,
    /// Other parameters, preserved as raw key=value strings.
    parameters: Vec<(String, String)>,
}

impl MediaTypeHeaderValue {
    /// Construct from a media type/subtype. Use `.with_charset(...)` or
    /// `.with_param(...)` to add parameters.
    pub fn new(type_: impl Into<String>, subtype: impl Into<String>) -> Self {
        Self {
            type_: type_.into(),
            subtype: subtype.into(),
            charset: None,
            parameters: Vec::new(),
        }
    }

    /// Parse a `Content-Type`-style header value. Accepts `text/html`,
    /// `text/html; charset=utf-8`, `application/json; charset="utf-8"`, etc.
    pub fn parse(s: &str) -> Result<Self, MediaTypeError> {
        let s = s.trim();
        let (type_subtype, params) = match s.split_once(';') {
            Some((ts, rest)) => (ts.trim(), rest),
            None => (s, ""),
        };
        let (type_, subtype) = type_subtype
            .split_once('/')
            .ok_or_else(|| MediaTypeError(format!("missing '/': {s}")))?;
        let type_ = type_.trim().to_ascii_lowercase();
        let subtype = subtype.trim().to_ascii_lowercase();
        if type_.is_empty() || subtype.is_empty() {
            return Err(MediaTypeError(format!("empty type or subtype: {s}")));
        }
        let mut charset = None;
        let mut parameters = Vec::new();
        for raw in params.split(';') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let (k, v) = raw
                .split_once('=')
                .ok_or_else(|| MediaTypeError(format!("malformed parameter: {raw}")))?;
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().trim_matches('"').to_string();
            if k == "charset" {
                charset = Some(v);
            } else {
                parameters.push((k, v));
            }
        }
        Ok(Self {
            type_,
            subtype,
            charset,
            parameters,
        })
    }

    /// The top-level type, e.g. `text`.
    pub fn media_type(&self) -> &str {
        &self.type_
    }

    /// The subtype, e.g. `html`.
    pub fn subtype(&self) -> &str {
        &self.subtype
    }

    /// The charset, if any.
    pub fn charset(&self) -> Option<&str> {
        self.charset.as_deref()
    }

    /// Builder: set the charset.
    #[must_use]
    pub fn with_charset(mut self, charset: impl Into<String>) -> Self {
        self.charset = Some(charset.into());
        self
    }

    /// Builder: add a parameter.
    #[must_use]
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.push((key.into(), value.into()));
        self
    }

    /// Render to a wire value, e.g. `text/html; charset=utf-8`.
    pub fn to_header_value(&self) -> HeaderValue {
        let mut s = format!("{}/{}", self.type_, self.subtype);
        if let Some(cs) = &self.charset {
            s.push_str("; charset=");
            s.push_str(cs);
        }
        for (k, v) in &self.parameters {
            s.push_str("; ");
            s.push_str(k);
            s.push('=');
            s.push_str(v);
        }
        HeaderValue::from_str(&s).expect("media type renders as a valid header value")
    }
}

impl fmt::Display for MediaTypeHeaderValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hv = self.to_header_value();
        f.write_str(hv.to_str().unwrap_or(""))
    }
}

impl FromStr for MediaTypeHeaderValue {
    type Err = MediaTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Error returned when a media type cannot be parsed.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct MediaTypeError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let m = MediaTypeHeaderValue::parse("text/html").unwrap();
        assert_eq!(m.media_type(), "text");
        assert_eq!(m.subtype(), "html");
        assert_eq!(m.charset(), None);
    }

    #[test]
    fn parse_with_charset() {
        let m = MediaTypeHeaderValue::parse("text/html; charset=utf-8").unwrap();
        assert_eq!(m.charset(), Some("utf-8"));
        assert_eq!(m.to_string(), "text/html; charset=utf-8");
    }

    #[test]
    fn parse_with_quoted_charset() {
        let m = MediaTypeHeaderValue::parse(r#"text/html; charset="utf-8""#).unwrap();
        assert_eq!(m.charset(), Some("utf-8"));
    }

    #[test]
    fn parse_unknown_parameter_preserved() {
        let m = MediaTypeHeaderValue::parse("multipart/form-data; boundary=----abc; charset=utf-8")
            .unwrap();
        assert_eq!(m.charset(), Some("utf-8"));
        assert_eq!(
            m.to_string(),
            "multipart/form-data; charset=utf-8; boundary=----abc"
        );
    }
}
