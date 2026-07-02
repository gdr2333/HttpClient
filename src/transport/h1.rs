//! HTTP/1.1 wire format: request line + headers + body, response status +
//! headers + body. Hand-rolled on top of `tokio::io` to keep the
//! dependency surface small. The `http` crate's `HeaderMap` / `StatusCode`
//! / `Method` are reused for the value types.
//!
//! Supported body transfer encodings:
//! - `Content-Length` (request and response)
//! - `Transfer-Encoding: chunked` (response; request uses Content-Length)
//!
//! Connection: `close` is honored, otherwise keep-alive is assumed.
//!
//! The current iteration reads the entire body into memory. Streaming is a
//! future enhancement; the trait boundary is preserved so it can be added
//! without breaking the public API.

use bytes::{Bytes, BytesMut};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Version};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::content::HttpContent;
use crate::error::HttpRequestException;

/// Parsed HTTP/1.1 response head (status + version + headers + buffered body).
#[derive(Debug, Clone)]
pub struct ResponseHead {
    pub status: StatusCode,
    pub version: Version,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Write an HTTP/1.1 request to `writer`, including the body. The
/// `Content-Length` header is added or overridden based on
/// `body.content_length()`. The `Host` header is added automatically if
/// not already present.
pub async fn write_request<W>(
    mut writer: W,
    method: &Method,
    path: &str,
    host: &str,
    headers: &HeaderMap,
    mut body: Option<&mut dyn HttpContent>,
) -> Result<(), HttpRequestException>
where
    W: AsyncWrite + Unpin + Send,
{
    // Request line.
    writer
        .write_all(method.as_str().as_bytes())
        .await
        .map_err(|e| HttpRequestException::new(format!("write request line: {e}"), None))?;
    writer
        .write_all(b" ")
        .await
        .map_err(|e| HttpRequestException::new(format!("write request line: {e}"), None))?;
    writer
        .write_all(path.as_bytes())
        .await
        .map_err(|e| HttpRequestException::new(format!("write request line: {e}"), None))?;
    writer
        .write_all(b" HTTP/1.1\r\n")
        .await
        .map_err(|e| HttpRequestException::new(format!("write request line: {e}"), None))?;

    // Host header (mandatory for HTTP/1.1).
    if !headers.contains_key(http::header::HOST) {
        writer
            .write_all(b"Host: ")
            .await
            .map_err(|e| HttpRequestException::new(format!("write Host: {e}"), None))?;
        writer
            .write_all(host.as_bytes())
            .await
            .map_err(|e| HttpRequestException::new(format!("write Host: {e}"), None))?;
        writer
            .write_all(b"\r\n")
            .await
            .map_err(|e| HttpRequestException::new(format!("write Host: {e}"), None))?;
    }

    // Headers from caller.
    for (name, value) in headers.iter() {
        write_header(&mut writer, name, value).await?;
    }

    // Body headers + Content-Length.
    let body_bytes = if let Some(b) = body.as_deref_mut() {
        let bytes = b.read_as_bytes().await?;
        if !headers.contains_key(http::header::CONTENT_LENGTH) {
            let len_str = bytes.len().to_string();
            writer
                .write_all(b"Content-Length: ")
                .await
                .map_err(|e| HttpRequestException::new(format!("write CL: {e}"), None))?;
            writer
                .write_all(len_str.as_bytes())
                .await
                .map_err(|e| HttpRequestException::new(format!("write CL: {e}"), None))?;
            writer
                .write_all(b"\r\n")
                .await
                .map_err(|e| HttpRequestException::new(format!("write CL: {e}"), None))?;
        }
        // Forward body content headers (e.g. Content-Type).
        for (name, value) in b.headers().iter() {
            if !headers.contains_key(name) {
                write_header(&mut writer, name, value).await?;
            }
        }
        writer
            .write_all(b"\r\n")
            .await
            .map_err(|e| HttpRequestException::new(format!("write CRLF: {e}"), None))?;
        Some(bytes)
    } else {
        writer
            .write_all(b"\r\n")
            .await
            .map_err(|e| HttpRequestException::new(format!("write CRLF: {e}"), None))?;
        None
    };

    if let Some(bytes) = body_bytes {
        writer
            .write_all(&bytes)
            .await
            .map_err(|e| HttpRequestException::new(format!("write body: {e}"), None))?;
    }
    writer
        .flush()
        .await
        .map_err(|e| HttpRequestException::new(format!("flush: {e}"), None))?;
    Ok(())
}

async fn write_header<W: AsyncWrite + Unpin + Send>(
    mut writer: W,
    name: &HeaderName,
    value: &HeaderValue,
) -> Result<(), HttpRequestException> {
    writer
        .write_all(name.as_str().as_bytes())
        .await
        .map_err(|e| HttpRequestException::new(format!("write header: {e}"), None))?;
    writer
        .write_all(b": ")
        .await
        .map_err(|e| HttpRequestException::new(format!("write header: {e}"), None))?;
    writer
        .write_all(value.as_bytes())
        .await
        .map_err(|e| HttpRequestException::new(format!("write header: {e}"), None))?;
    writer
        .write_all(b"\r\n")
        .await
        .map_err(|e| HttpRequestException::new(format!("write header: {e}"), None))?;
    Ok(())
}

/// Read an HTTP/1.1 response from `reader` into a buffered `ResponseHead`.
/// The body is consumed and returned as `Bytes`.
pub async fn read_response<R>(reader: R) -> Result<ResponseHead, HttpRequestException>
where
    R: AsyncRead + Unpin + Send,
{
    let mut reader = BufReader::new(reader);

    // Status line: HTTP/1.1 200 OK
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .await
        .map_err(|e| HttpRequestException::new(format!("read status line: {e}"), None))?;
    let status_line = status_line.trim_end_matches(['\r', '\n']);
    let mut parts = status_line.splitn(3, ' ');
    let version_str = parts
        .next()
        .ok_or_else(|| HttpRequestException::new("missing HTTP version", None))?;
    let code_str = parts
        .next()
        .ok_or_else(|| HttpRequestException::new("missing status code", None))?;
    let _reason = parts.next().unwrap_or("");
    let version = match version_str {
        "HTTP/1.1" => Version::HTTP_11,
        "HTTP/1.0" => Version::HTTP_10,
        other => {
            return Err(HttpRequestException::new(
                format!("unsupported HTTP version: {other}"),
                None,
            ));
        }
    };
    let status = StatusCode::from_bytes(code_str.as_bytes()).map_err(|e| {
        HttpRequestException::new(format!("invalid status code {code_str}: {e}"), None)
    })?;

    // Headers.
    let mut headers = HeaderMap::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| HttpRequestException::new(format!("read header: {e}"), None))?;
        if n == 0 {
            return Err(HttpRequestException::new("unexpected EOF in headers", None));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            HttpRequestException::new(format!("malformed header line: {line}"), None)
        })?;
        let name = name.trim();
        let value = value.trim();
        let hn = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
            HttpRequestException::new(format!("invalid header name {name}: {e}"), None)
        })?;
        let hv = HeaderValue::from_str(value).map_err(|e| {
            HttpRequestException::new(format!("invalid header value {value}: {e}"), None)
        })?;
        headers.append(hn, hv);
    }

    // Body framing.
    let transfer_encoding = headers
        .get(http::header::TRANSFER_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase());
    let content_length = headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let connection_close = headers
        .get(http::header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("close"))
        .unwrap_or(false);

    let body = if transfer_encoding.as_deref() == Some("chunked") {
        read_chunked(&mut reader).await?
    } else if let Some(len) = content_length {
        let mut buf = BytesMut::with_capacity(len as usize);
        buf.resize(len as usize, 0);
        reader
            .read_exact(&mut buf)
            .await
            .map_err(|e| HttpRequestException::new(format!("read body: {e}"), None))?;
        buf.freeze()
    } else if status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
        || matches!(status.as_u16(), 100..=199)
    {
        Bytes::new()
    } else if connection_close {
        // Read until EOF.
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf)
            .await
            .map_err(|e| HttpRequestException::new(format!("read body: {e}"), None))?;
        Bytes::from(buf)
    } else {
        Bytes::new()
    };

    Ok(ResponseHead {
        status,
        version,
        headers,
        body,
    })
}

async fn read_chunked<R: AsyncRead + Unpin + Send>(
    reader: &mut BufReader<R>,
) -> Result<Bytes, HttpRequestException> {
    let mut out = BytesMut::new();
    loop {
        let mut size_line = String::new();
        reader
            .read_line(&mut size_line)
            .await
            .map_err(|e| HttpRequestException::new(format!("read chunk size: {e}"), None))?;
        let size_line = size_line.trim_end_matches(['\r', '\n']);
        let size_str = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|e| HttpRequestException::new(format!("bad chunk size {size_str}: {e}"), None))?;
        if size == 0 {
            // Consume trailers + final CRLF.
            loop {
                let mut line = String::new();
                let n = reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| HttpRequestException::new(format!("read trailer: {e}"), None))?;
                if n == 0 || line.trim_end_matches(['\r', '\n']).is_empty() {
                    break;
                }
            }
            break;
        }
        let mut chunk = vec![0u8; size];
        reader
            .read_exact(&mut chunk)
            .await
            .map_err(|e| HttpRequestException::new(format!("read chunk data: {e}"), None))?;
        out.extend_from_slice(&chunk);
        // Trailing CRLF after chunk.
        let mut crlf = [0u8; 2];
        let _ = reader
            .read_exact(&mut crlf)
            .await
            .map_err(|e| HttpRequestException::new(format!("read chunk CRLF: {e}"), None))?;
    }
    Ok(out.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn write_request_smoke() {
        let (a, mut b) = duplex(128);
        let headers = HeaderMap::new();
        let fut = async {
            write_request(a, &Method::GET, "/", "example.com", &headers, None)
                .await
                .unwrap();
        };
        fut.await;
        let mut out = Vec::new();
        let _ = b.read_to_end(&mut out).await.unwrap();
        let s = std::str::from_utf8(&out).unwrap_or("");
        assert!(s.starts_with("GET / HTTP/1.1\r\n"));
        assert!(s.contains("Host: example.com"));
    }

    #[tokio::test]
    async fn read_response_parses_204() {
        let raw: &[u8] = b"HTTP/1.1 204 No Content\r\nServer: test\r\n\r\n";
        let head = read_response(raw).await.unwrap();
        assert_eq!(head.status, StatusCode::NO_CONTENT);
        assert_eq!(
            head.headers.get(http::header::SERVER).unwrap(),
            &HeaderValue::from_static("test")
        );
        assert_eq!(head.body.len(), 0);
    }

    #[tokio::test]
    async fn read_response_parses_chunked() {
        let raw: &[u8] =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
        let head = read_response(&raw[..]).await.unwrap();
        assert_eq!(head.status, StatusCode::OK);
        assert_eq!(&head.body[..], b"hello");
    }
}
