//! The actual send logic: open a TCP connection (with or without TLS), write
//! the request, read the response, follow redirects if enabled.

use http::header::HeaderName;
use http::HeaderMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::OnceCell;
use tokio_rustls::TlsConnector;
use url::Url;

use crate::cancellation::CancellationToken;
use crate::content::HttpContent;
use crate::error::{HttpRequestError, HttpRequestException, OperationCanceledException};
use crate::handlers::sockets::SocketsHttpHandler;
use crate::request::HttpRequestMessage;
use crate::response::HttpResponseMessage;
use crate::transport::h1;
use crate::transport::pool::{PoolKey, Scheme};
use crate::transport::tls;

/// Cached, process-wide TLS connector. The TLS configuration never changes
/// at runtime, so we only build it once.
static TLS_CONNECTOR: OnceCell<TlsConnector> = OnceCell::const_new();

async fn get_tls_connector() -> &'static TlsConnector {
    TLS_CONNECTOR
        .get_or_init(|| async { tls::build_tls_connector() })
        .await
}

/// Run a `SendAsync` for the given handler. Public-in-crate entry point
/// from `SocketsHttpHandler::send_async`.
pub async fn execute_send(
    handler: &SocketsHttpHandler,
    request: HttpRequestMessage,
    cancellation_token: CancellationToken,
) -> Result<HttpResponseMessage, HttpRequestError> {
    let _ = handler;
    let mut request = request;
    let request_headers = request.headers().as_map().clone();
    let url = match resolve_url(&request) {
        Ok(u) => u,
        Err(e) => return Err(HttpRequestError::Http(e)),
    };

    let mut method = request.method().clone();
    let mut uri = url;
    let mut redirections_left = handler.max_redirections;
    let allow_auto_redirect = handler.allow_auto_redirect;
    let cancel = cancellation_token.clone();

    loop {
        let path_and_query = build_path_and_query(&uri);
        let host = uri
            .host_str()
            .ok_or_else(|| {
                HttpRequestError::Http(HttpRequestException::new("URL has no host", None))
            })?
            .to_string();
        let port = uri.port_or_known_default().unwrap_or(80);
        let scheme = match uri.scheme() {
            "https" => Scheme::Https,
            _ => Scheme::Http,
        };
        let host_header = host_with_port(&host, port, scheme);
        let _ = PoolKey {
            scheme,
            host: host.clone(),
            port,
        };

        let http_method: http::Method = method.as_str().parse().map_err(|_| {
            HttpRequestError::Http(HttpRequestException::new(
                format!("invalid method: {method}"),
                None,
            ))
        })?;
        let headers = request_headers.clone();

        // Take ownership of the body so we don't hold a borrow on `request`
        // across the await. `take_content` is `Option::take`, so this is
        // O(1) and the borrow on `request` ends immediately.
        let body: Option<Box<dyn HttpContent>> = request.take_content();

        let response = {
            let send_fut = send_once(
                scheme,
                host.clone(),
                port,
                &http_method,
                &path_and_query,
                &host_header,
                &headers,
                body,
            );
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(HttpRequestError::Canceled(OperationCanceledException::new("canceled")));
                }
                r = send_fut => r?,
            }
        };

        let should_redirect = allow_auto_redirect
            && redirections_left > 0
            && matches!(response.status_code().as_u16(), 301..=308)
            && !matches!(response.status_code().as_u16(), 304..=306);
        if !should_redirect {
            return Ok(response);
        }

        let location = response
            .headers()
            .as_map()
            .get(HeaderName::from_static("location"))
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let Some(location) = location else {
            return Ok(response);
        };
        let new_uri = match uri.join(&location) {
            Ok(u) => u,
            Err(_) => return Ok(response),
        };
        uri = new_uri;

        let original_method = method.clone();
        method = match method {
            crate::method::HttpMethod::Post | crate::method::HttpMethod::Put
                if matches!(response.status_code().as_u16(), 301..=303) =>
            {
                crate::method::HttpMethod::Get
            }
            other => other,
        };
        if std::mem::discriminant(&method) != std::mem::discriminant(&original_method) {
            // Method changed; the body has already been taken on the
            // previous send, so the next iteration sends no body.
        }
        redirections_left = redirections_left.saturating_sub(1);
    }
}

fn resolve_url(request: &HttpRequestMessage) -> Result<Url, HttpRequestException> {
    if let Some(uri) = request.request_uri() {
        Ok(uri.as_url().clone())
    } else {
        Err(HttpRequestException::new(
            "request has no URI and HttpClient.BaseAddress is not set",
            None,
        ))
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_once(
    scheme: Scheme,
    host: String,
    port: u16,
    method: &http::Method,
    path_and_query: &str,
    host_header: &str,
    headers: &HeaderMap,
    body: Option<Box<dyn HttpContent>>,
) -> Result<HttpResponseMessage, HttpRequestError> {
    match scheme {
        Scheme::Http => {
            let tcp = open_tcp(&host, port).await?;
            let (mut reader, mut writer) = tokio::io::split(tcp);
            h1::write_request(
                &mut writer,
                method,
                path_and_query,
                host_header,
                headers,
                body,
            )
            .await
            .map_err(HttpRequestError::Http)?;
            let head = h1::read_response(&mut reader)
                .await
                .map_err(HttpRequestError::Http)?;
            Ok(build_response(head))
        }
        Scheme::Https => {
            let tcp = open_tcp(&host, port).await?;
            let server_name =
                rustls::pki_types::ServerName::try_from(host.clone()).map_err(|e| {
                    HttpRequestError::Http(HttpRequestException::new(
                        format!("invalid server name {host}: {e}"),
                        None,
                    ))
                })?;
            let tls = get_tls_connector()
                .await
                .connect(server_name, tcp)
                .await
                .map_err(|e| {
                    HttpRequestError::Http(HttpRequestException::new(
                        format!("tls handshake: {e}"),
                        None,
                    ))
                })?;
            let (mut reader, mut writer) = tokio::io::split(tls);
            h1::write_request(
                &mut writer,
                method,
                path_and_query,
                host_header,
                headers,
                body,
            )
            .await
            .map_err(HttpRequestError::Http)?;
            let head = h1::read_response(&mut reader)
                .await
                .map_err(HttpRequestError::Http)?;
            Ok(build_response(head))
        }
    }
}

fn build_response(head: h1::ResponseHead) -> HttpResponseMessage {
    let mut response = HttpResponseMessage::new(head.status);
    response.set_version(head.version);
    for (k, v) in head.headers.iter() {
        response.headers_mut().set(k.clone(), v.clone());
    }
    let ct = response
        .headers()
        .as_map()
        .get(HeaderName::from_static("content-type"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = crate::content::ByteArrayContent::with_media_type(
        head.body,
        ct.as_deref().unwrap_or("application/octet-stream"),
    );
    response.set_content(Box::new(bytes));
    response
}

fn host_with_port(host: &str, port: u16, scheme: Scheme) -> String {
    let default_port = match scheme {
        Scheme::Http => 80,
        Scheme::Https => 443,
    };
    if port == default_port {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

fn build_path_and_query(url: &Url) -> String {
    let mut s = url.path().to_string();
    if let Some(q) = url.query() {
        s.push('?');
        s.push_str(q);
    }
    if s.is_empty() {
        s.push('/');
    }
    s
}

async fn open_tcp(host: &str, port: u16) -> Result<TcpStream, HttpRequestError> {
    let tcp = TcpStream::connect((host, port)).await.map_err(|e| {
        HttpRequestError::Http(HttpRequestException::new(
            format!("tcp connect {host}:{port}: {e}"),
            None,
        ))
    })?;
    tcp.set_nodelay(true).ok();
    Ok(tcp)
}

#[allow(dead_code)]
fn _unused_anchor<T: AsyncRead + Unpin + Send>(_: T) {}
#[allow(dead_code)]
fn _unused_writer<T: AsyncWrite + Unpin + Send>(_: T) {}
#[allow(dead_code)]
fn _unused_writer2<T: AsyncWriteExt + Unpin + Send>(_: T) {}
#[allow(dead_code)]
fn _unused_reader2<T: AsyncReadExt + Unpin + Send>(_: T) {}
