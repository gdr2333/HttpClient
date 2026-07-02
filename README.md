# httpclient

A C#-style `HttpClient` API surface for async Rust, backed by tokio + rustls.

The crate mirrors the public shape of `System.Net.Http.HttpClient` so callers
familiar with the .NET model can map concepts 1:1: `HttpClient`,
`HttpRequestMessage`, `HttpResponseMessage`, `HttpMethod`, `HttpHeaders`,
`HttpContent` (and its subtypes), `CancellationToken`. Internally the
transport is HTTP/1.1 over tokio TCP, with HTTPS via rustls.

## Scope

This iteration covers the **core** C# surface:

- `HttpClient` with `BaseAddress`, `Timeout`, `DefaultRequestVersion`,
  `DefaultRequestHeaders`
- `GetAsync` / `GetStringAsync` / `GetByteArrayAsync` / `GetStreamAsync`
- `PostAsync` / `PutAsync` / `DeleteAsync` / `PatchAsync` / `HeadAsync` /
  `OptionsAsync`
- `SendAsync(HttpRequestMessage, CancellationToken)`
- `HttpRequestMessage` with `Method`, `RequestUri`, `Version`,
  `VersionPolicy`, `Headers`, `Content`
- `HttpResponseMessage` with `StatusCode`, `Version`, `Headers`, `Content`,
  `IsSuccessStatusCode`, `EnsureSuccessStatusCode()`
- `HttpMethod` (Get, Post, Put, Delete, Head, Options, Patch, Trace, Connect,
  Custom)
- `HttpHeaders` / `HttpRequestHeaders` / `HttpResponseHeaders` /
  `HttpContentHeaders` (case-insensitive, multi-valued)
- `HttpContent` trait + `StringContent`, `ByteArrayContent`,
  `StreamContent`, `FormUrlEncodedContent`, `MultipartContent`,
  `MultipartFormDataContent`
- `CancellationToken` with `none`, `new`, `child_token`, `link_with`,
  `with_timeout`
- `HttpVersion` + `HttpVersionPolicy`
- `MediaTypeHeaderValue`
- `HttpRequestException` (mirrors C#'s `HttpRequestException`) and
  `OperationCanceledException`

**Out of scope (this iteration)**: HTTP/2, HTTP/3, `DelegatingHandler`
middleware, proxies, cookies, authentication handlers, request retries.

## Quickstart

```rust
use httpclient::{HttpClient, CancellationToken, StringContent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let mut response = client
        .get_async("https://httpbin.org/get", CancellationToken::none())
        .await?;
    println!("status: {}", response.status_code());
    let body = response.content_mut().read_as_string().await?;
    println!("{body}");
    Ok(())
}
```

With a body and POST:

```rust
use httpclient::{HttpClient, CancellationToken, FormUrlEncodedContent};

let client = HttpClient::new();
let form = FormUrlEncodedContent::new(vec![("a", "1"), ("b", "hello world")]);
let mut response = client
    .post_async(
        "https://httpbin.org/post",
        Box::new(form),
        CancellationToken::none(),
    )
    .await?;
let body = response.content_mut().read_as_string().await?;
```

## Builder

```rust
use httpclient::HttpClientBuilder;
use std::time::Duration;
use httpclient::uri::Uri;

let client = HttpClientBuilder::new()
    .base_address(Some(Uri::parse("https://api.example.com/v1/")?))
    .timeout(Duration::from_secs(30))
    .build();
```

## Cancellation / timeout

```rust
use std::time::Duration;
use httpclient::CancellationToken;

let token = CancellationToken::new().with_timeout(Duration::from_secs(5));
let response = client.get_async("https://api.example.com/data", token).await;
// Returns `Err(HttpRequestError::Canceled(OperationCanceledException))` on timeout.
```

## Examples

Run any of these from the crate root:

```bash
cargo run --example basic_get
cargo run --example post_string
cargo run --example post_form
```

## Tests

```bash
cargo test
```

The test suite is **self-contained** — it uses an in-process HTTP/1.1 test
server (`tests/support/server.rs`) that listens on `127.0.0.1:0` and records
incoming requests for assertion. No external network, no external test
endpoints, no `curl`.

Coverage:
- 32 unit tests across `HttpMethod`, `HttpHeaders`, `Uri`, `MediaTypeHeaderValue`,
  `CancellationToken`, `HttpRequestException`, and the `transport/h1` codec.
- 6 integration tests using the in-process server: GET shape, POST form,
  POST string, byte-array body, redirect chain, and timeout cancellation.

## License

Apache-2.0
