//! In-process HTTP/1.1 test server.
//!
//! A small `tokio::net::TcpListener` that:
//! - speaks HTTP/1.1,
//! - replies with a configurable status / headers / body,
//! - records the most-recent incoming request line + headers for
//!   assertions,
//! - supports scripted behaviors (delays, redirects, custom paths).
//!
//! The point is to give the test suite a self-contained correctness gate
//! that doesn't depend on any external network. No curl, no
//! `python -m http.server`, no public test endpoints.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// A scripted behavior: what to do when a request comes in.
#[derive(Debug, Clone)]
pub enum ScriptedResponse {
    /// Reply with a fixed status + headers + body.
    Fixed {
        status: u16,
        reason: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    /// Reply with a redirect to the given `Location`.
    Redirect {
        status: u16,
        location: String,
    },
    /// Sleep for `delay` before responding. Used to test timeouts.
    DelayThenFixed {
        delay: Duration,
        status: u16,
        reason: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

impl ScriptedResponse {
    /// Default: 200 OK with `text/plain` body.
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::Fixed {
            status: 200,
            reason: "OK".to_string(),
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: body.into(),
        }
    }

    /// 301 with a Location header.
    pub fn redirect_301(location: impl Into<String>) -> Self {
        Self::Redirect {
            status: 301,
            location: location.into(),
        }
    }
}

/// The shared state of the server.
#[derive(Debug, Default)]
struct ServerState {
    /// The most-recent request line + headers + body, for assertions.
    pub last_request: Option<RecordedRequest>,
    /// Path -> scripted response.
    pub routes: HashMap<String, ScriptedResponse>,
}

/// A recorded incoming request.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    /// The full request line, e.g. `GET /foo HTTP/1.1`.
    pub request_line: String,
    /// Method, e.g. `GET`.
    pub method: String,
    /// Path, e.g. `/foo`.
    pub path: String,
    /// Headers, in arrival order.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

/// A handle to the in-process test server. The server runs in a background
/// task; `addr` is its bound address (typically `127.0.0.1:0`).
#[derive(Debug, Clone)]
pub struct TestServer {
    state: Arc<Mutex<ServerState>>,
    addr: SocketAddr,
}

impl TestServer {
    /// Start a new server. Binds to a free port on `127.0.0.1`.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(ServerState::default()));
        let state_clone = state.clone();
        tokio::spawn(async move {
            run(listener, state_clone).await;
        });
        Self { state, addr }
    }

    /// The local address the server is listening on. `http://{addr}` works.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// A `http://` URL with the given path.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    /// Register a response for a given path.
    pub async fn route(&self, path: &str, response: ScriptedResponse) {
        let mut s = self.state.lock().await;
        s.routes.insert(path.to_string(), response);
    }

    /// The most-recent recorded request, if any.
    pub async fn last_request(&self) -> Option<RecordedRequest> {
        self.state.lock().await.last_request.clone()
    }
}

async fn run(listener: TcpListener, state: Arc<Mutex<ServerState>>) {
    loop {
        let (mut sock, _peer) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(_) = handle(&mut sock, &state).await {
                // Connection ended or malformed; ignore.
            }
        });
    }
}

async fn handle(
    sock: &mut tokio::net::TcpStream,
    state: &Arc<Mutex<ServerState>>,
) -> std::io::Result<()> {
    // Read until end of headers.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let header_end;
    loop {
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_double_crlf(&buf) {
            header_end = pos + 4;
            break;
        }
    }

    // Parse request line + headers.
    let head_str = std::str::from_utf8(&buf[..header_end - 4])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut lines = head_str.split("\r\n");
    let request_line = lines.next().unwrap_or("").to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    // Read body if Content-Length is present.
    let content_length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    let recorded = RecordedRequest {
        request_line: request_line.clone(),
        method: method.clone(),
        path: path.clone(),
        headers: headers.clone(),
        body: body.clone(),
    };

    // Pick a response.
    let response = {
        let mut s = state.lock().await;
        s.last_request = Some(recorded);
        s.routes.get(&path).cloned().unwrap_or_else(|| {
            ScriptedResponse::Fixed {
                status: 404,
                reason: "Not Found".to_string(),
                headers: vec![("Content-Length".to_string(), "0".to_string())],
                body: Vec::new(),
            }
        })
    };

    match response {
        ScriptedResponse::Fixed {
            status,
            reason,
            headers,
            body,
        } => {
            // Auto-add Content-Length if not provided.
            let mut h = headers.clone();
            if !h.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-length")) {
                h.push(("Content-Length".to_string(), body.len().to_string()));
            }
            let resp = build_response(status, &reason, &h, &body);
            sock.write_all(&resp).await?;
        }
        ScriptedResponse::Redirect { status, location } => {
            let resp = build_response(
                status,
                "Moved Permanently",
                &[("Location".to_string(), location), ("Content-Length".to_string(), "0".to_string())],
                &[],
            );
            sock.write_all(&resp).await?;
        }
        ScriptedResponse::DelayThenFixed {
            delay,
            status,
            reason,
            headers,
            body,
        } => {
            tokio::time::sleep(delay).await;
            let mut h = headers.clone();
            if !h.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-length")) {
                h.push(("Content-Length".to_string(), body.len().to_string()));
            }
            let resp = build_response(status, &reason, &h, &body);
            sock.write_all(&resp).await?;
        }
    }
    sock.shutdown().await.ok();
    Ok(())
}

fn build_response(
    status: u16,
    reason: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut s = format!("HTTP/1.1 {status} {reason}\r\n");
    for (k, v) in headers {
        s.push_str(k);
        s.push_str(": ");
        s.push_str(v);
        s.push_str("\r\n");
    }
    s.push_str("\r\n");
    let mut out = s.into_bytes();
    out.extend_from_slice(body);
    out
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}
