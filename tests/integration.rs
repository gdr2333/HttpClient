//! Integration tests against the in-process test server.

use std::time::Duration;

use httpclient::{
    ByteArrayContent, CancellationToken, FormUrlEncodedContent, HttpClient, StringContent,
};

mod support;
use support::server::{ScriptedResponse, TestServer};

#[tokio::test]
async fn get_request_lands_with_expected_method_path_host() {
    let server = TestServer::start().await;
    server.route("/echo", ScriptedResponse::ok("hello")).await;
    let client = HttpClient::new();
    let mut response = client
        .get_async(server.url("/echo"), CancellationToken::none())
        .await
        .unwrap();
    assert_eq!(response.status_code().as_u16(), 200);
    let body = response.content_mut().read_as_bytes().await.unwrap();
    assert_eq!(&body[..], b"hello");

    let recorded = server.last_request().await.unwrap();
    assert_eq!(recorded.method, "GET");
    assert_eq!(recorded.path, "/echo");
    let host_header = recorded
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert!(host_header.starts_with("127.0.0.1"));
}

#[tokio::test]
async fn post_form_sends_content_length_and_url_encoded_body() {
    let server = TestServer::start().await;
    server.route("/post", ScriptedResponse::ok("ok")).await;
    let client = HttpClient::new();
    let form = FormUrlEncodedContent::new(vec![("a", "1"), ("b", "hello world")]);
    let response = client
        .post_async(
            server.url("/post"),
            Box::new(form),
            CancellationToken::none(),
        )
        .await
        .unwrap();
    assert_eq!(response.status_code().as_u16(), 200);

    let recorded = server.last_request().await.unwrap();
    assert_eq!(recorded.method, "POST");
    let cl = recorded
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    let ct = recorded
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert_eq!(ct, "application/x-www-form-urlencoded");
    assert_eq!(cl, "17"); // a=1&b=hello+world (17 bytes)
    assert_eq!(recorded.body, b"a=1&b=hello+world");
}

#[tokio::test]
async fn post_string_uses_provided_content_type() {
    let server = TestServer::start().await;
    server.route("/post", ScriptedResponse::ok("ok")).await;
    let client = HttpClient::new();
    let content = StringContent::new("hi");
    let _ = client
        .post_async(
            server.url("/post"),
            Box::new(content),
            CancellationToken::none(),
        )
        .await
        .unwrap();
    let recorded = server.last_request().await.unwrap();
    let ct = recorded
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert!(ct.starts_with("text/plain"));
    assert!(ct.contains("utf-8"));
    assert_eq!(recorded.body, b"hi");
}

#[tokio::test]
async fn byte_array_body_lands() {
    let server = TestServer::start().await;
    server.route("/upload", ScriptedResponse::ok("ok")).await;
    let client = HttpClient::new();
    let content = ByteArrayContent::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let _ = client
        .post_async(
            server.url("/upload"),
            Box::new(content),
            CancellationToken::none(),
        )
        .await
        .unwrap();
    let recorded = server.last_request().await.unwrap();
    assert_eq!(recorded.body, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[tokio::test]
async fn redirect_is_followed() {
    let server = TestServer::start().await;
    server
        .route("/start", ScriptedResponse::redirect_301("/end"))
        .await;
    server.route("/end", ScriptedResponse::ok("landed")).await;
    let client = HttpClient::new();
    let mut response = client
        .get_async(server.url("/start"), CancellationToken::none())
        .await
        .unwrap();
    assert_eq!(response.status_code().as_u16(), 200);
    let body = response.content_mut().read_as_bytes().await.unwrap();
    assert_eq!(&body[..], b"landed");
}

#[tokio::test]
async fn timeout_cancels_long_request() {
    let server = TestServer::start().await;
    server
        .route(
            "/slow",
            ScriptedResponse::DelayThenFixed {
                delay: Duration::from_secs(2),
                status: 200,
                reason: "OK".to_string(),
                headers: vec![("Content-Length".to_string(), "2".to_string())],
                body: b"ok".to_vec(),
            },
        )
        .await;
    let client = HttpClient::new();
    let token = CancellationToken::new().with_timeout(Duration::from_millis(100));
    let result = client.get_async(server.url("/slow"), token).await;
    let err = result.expect_err("expected cancellation error");
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("cancel") || msg.to_lowercase().contains("timeout"),
        "unexpected error: {msg}"
    );
}
