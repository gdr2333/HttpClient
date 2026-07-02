//! `FormUrlEncodedContent` — `application/x-www-form-urlencoded` body.

use std::borrow::Cow;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::AsyncWrite;

use crate::content::HttpContent;
use crate::error::HttpRequestException;
use crate::headers::HttpContentHeaders;

/// A body of `key=value&key=value` pairs, equivalent to C#'s
/// `FormUrlEncodedContent`.
#[derive(Debug, Clone)]
pub struct FormUrlEncodedContent {
    /// `(name, value)` pairs in insertion order.
    pairs: Vec<(String, String)>,
    headers: HttpContentHeaders,
    encoded: Bytes,
}

impl FormUrlEncodedContent {
    /// Create from a list of `(name, value)` pairs. Both are URL-encoded.
    pub fn new<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let pairs: Vec<(String, String)> = pairs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let encoded = encode_pairs(&pairs);
        let mut headers = HttpContentHeaders::new();
        headers.set(
            http::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        );
        Self {
            pairs,
            headers,
            encoded,
        }
    }

    /// The encoded bytes (e.g. `a=1&b=2`).
    pub fn encoded(&self) -> &Bytes {
        &self.encoded
    }

    /// The original pairs.
    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }
}

fn encode_pairs(pairs: &[(String, String)]) -> Bytes {
    let mut s = String::new();
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            s.push('&');
        }
        s.push_str(&percent_encode(k));
        s.push('=');
        s.push_str(&percent_encode(v));
    }
    Bytes::from(s)
}

fn percent_encode(s: &str) -> Cow<'_, str> {
    let mut out = String::new();
    for &b in s.as_bytes() {
        match b {
            // Unreserved characters per RFC 3986.
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    Cow::Owned(out)
}

#[async_trait]
impl HttpContent for FormUrlEncodedContent {
    fn headers(&self) -> &HttpContentHeaders {
        &self.headers
    }
    fn content_length(&self) -> Option<u64> {
        Some(self.encoded.len() as u64)
    }
    async fn write_to(
        &mut self,
        mut writer: Pin<&mut (dyn AsyncWrite + Send + Unpin)>,
    ) -> Result<(), HttpRequestException> {
        use tokio::io::AsyncWriteExt;
        writer
            .write_all(&self.encoded)
            .await
            .map_err(|e| HttpRequestException::new(format!("write failed: {e}"), None))?;
        Ok(())
    }
    async fn read_as_bytes(&mut self) -> Result<Bytes, HttpRequestException> {
        Ok(self.encoded.clone())
    }
    fn try_clone(&self) -> Option<Box<dyn HttpContent>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_pairs() {
        let f = FormUrlEncodedContent::new(vec![("a", "1"), ("b", "hello world")]);
        assert_eq!(&f.encoded()[..], b"a=1&b=hello+world");
    }

    #[test]
    fn encodes_reserved_chars() {
        let f = FormUrlEncodedContent::new(vec![("k", "a&b=c")]);
        assert_eq!(&f.encoded()[..], b"k=a%26b%3Dc");
    }

    #[test]
    fn preserves_unreserved() {
        let f = FormUrlEncodedContent::new(vec![("AZaz09-_.~", "AZaz09-_.~")]);
        assert_eq!(&f.encoded()[..], b"AZaz09-_.~=AZaz09-_.~");
    }
}
