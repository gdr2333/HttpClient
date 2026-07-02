# CLAUDE.md

Project notes for `httpclient` — a C#-style `HttpClient` API surface for async
Rust. **Read this before changing anything** — the crate has design choices
that aren't obvious from the code alone.

## What this crate is

A re-implementation of C#'s `System.Net.Http.HttpClient` API in idiomatic
async Rust. The public types (`HttpClient`, `HttpRequestMessage`,
`HttpResponseMessage`, `HttpMethod`, `HttpHeaders`, `HttpContent`,
`CancellationToken`, `HttpVersion`, `HttpVersionPolicy`,
`MediaTypeHeaderValue`, `HttpRequestException`, `OperationCanceledException`)
mirror the C# model so callers can map concepts 1:1. The internals are HTTP/1.1
over tokio TCP, with TLS via rustls.

## Scope (this iteration)

In scope:
- `HttpClient` facade with `BaseAddress`, `Timeout`, `DefaultRequestVersion`,
  `DefaultRequestHeaders`
- All the convenience methods: `GetAsync` / `GetStringAsync` /
  `GetByteArrayAsync` / `GetStreamAsync` / `PostAsync` / `PutAsync` /
  `DeleteAsync` / `PatchAsync` / `HeadAsync` / `OptionsAsync` + the
  low-level `SendAsync`
- `HttpRequestMessage`, `HttpResponseMessage` with the C# shape
- `HttpMethod` (Get/Post/Put/Delete/Head/Options/Patch/Trace/Connect + Custom)
- `HttpHeaders` (case-insensitive) and the three typed sub-collections
- `HttpContent` trait + `StringContent`, `ByteArrayContent`, `StreamContent`,
  `FormUrlEncodedContent`, `MultipartContent`, `MultipartFormDataContent`
- `CancellationToken` with `none`, `new`, `child_token`, `link_with`,
  `with_timeout`
- `HttpVersion` + `HttpVersionPolicy`
- `MediaTypeHeaderValue`
- `HttpRequestException` + `OperationCanceledException`
- Auto-redirect for 3xx (POST/PUT → GET on 301/302/303, default)

Out of scope (explicitly):
- HTTP/2, HTTP/3, ALPN negotiation
- `DelegatingHandler` middleware chain
- Proxies (`HttpClientHandler.Proxy`)
- Cookies (`CookieContainer`)
- Authentication handlers
- Request retries
- Server-side / WebSocket

## Module map

```
src/
  lib.rs                — public re-exports, crate-level docs
  client.rs             — HttpClient + HttpClientBuilder
  request.rs            — HttpRequestMessage
  response.rs           — HttpResponseMessage
  method.rs             — HttpMethod
  headers.rs            — HttpHeaders + typed sub-collections
  uri.rs                — Uri (url::Url wrapper with relative-resolution)
  version.rs            — HttpVersion + HttpVersionPolicy
  media_type.rs         — MediaTypeHeaderValue
  cancellation.rs       — CancellationToken (wraps tokio_util)
  error.rs              — HttpRequestError, HttpRequestException,
                          OperationCanceledException
  content/
    mod.rs              — HttpContent trait (Send, body read/write)
    byte_array.rs       — ByteArrayContent
    string.rs           — StringContent
    form.rs             — FormUrlEncodedContent
    stream.rs           — StreamContent (lazy AsyncRead source)
    multipart.rs        — MultipartContent + MultipartFormDataContent
  handlers/
    mod.rs              — module entry
    handler.rs          — HttpMessageHandler trait
    sockets.rs          — SocketsHttpHandler (default)
    execute.rs          — actual send logic, redirect follow
  transport/
    mod.rs              — module entry
    h1.rs               — hand-rolled HTTP/1.1 codec
    tls.rs              — rustls connector + webpki-roots loader
    pool.rs             — connection pool bookkeeping (placeholder for now)

tests/
  integration.rs        — 6 in-process integration tests
  support/
    mod.rs
    server.rs           — in-process HTTP/1.1 test server

examples/
  basic_get.rs          — GET to httpbin.org/get
  post_string.rs        — POST StringContent to httpbin.org/anything
  post_form.rs          — POST FormUrlEncodedContent to httpbin.org/post
```

## Design notes

### `HttpContent` methods take `&mut self`

The trait methods `read_as_bytes`, `read_as_string`, and `write_to` all take
`&mut self`. This is deliberate: bodies can be consumed (a streaming body
once read is gone), and requiring `&mut self` makes the future returned be
`Send` without requiring the body to be `Sync`. Callers that hold a
`HttpResponseMessage` and want to read its body must declare it `mut`:

```rust
let mut response = client.get_async(...).await?;
let body = response.content_mut().read_as_string().await?;
```

### `Send`, not `Send + Sync`

`HttpContent: Send` (no `Sync` bound). A streaming body holds an
`AsyncRead` source that is not `Sync`, so adding `Sync` would force
`StreamContent` to use an async `Mutex`, which is overkill for the common
case. The trade-off: bodies can't be shared by `&` across threads, but they
can be sent (`tokio::spawn` works) and read by ownership.

### `Box<dyn HttpContent + 'static>`

The body field is `Option<Box<dyn HttpContent + 'static>>` (the `+ 'static`
is the default for trait objects in `Box`). The `content_mut()` method on
`HttpRequestMessage` returns `Option<&mut (dyn HttpContent + 'static)>`. The
`+ 'static` is necessary because of the variance of mutable references
combined with `Box`'s default lifetime bound.

### TLS provider is `ring`, not `aws_lc_rs`

The host this was built on has no `cmake`/`nasm`/`cl` on the PATH, so
`aws_lc_rs` (the default for rustls 0.23) would not build. We pin `ring`
in `Cargo.toml`. To switch to `aws_lc_rs` later, change the
`features = ["logging", "std", "ring", "tls12"]` line on `rustls` to
`["logging", "std", "aws_lc_rs", "tls12"]` and the corresponding line on
`tokio-rustls`.

### Connection pool is bookkeeping-only

`ConnectionPool` exists and tracks idle/active counts, but the transport
**always opens a new connection per request** for now. The pool will hold
real streams once we add per-stream expiry and rotation; the data structures
are in place.

### `webpki-roots`, not `rustls-native-certs`

We use Mozilla's bundled CA roots for reproducibility and to avoid platform-
specific cert-store parsing bugs. Switching to the host trust store is a
one-line change in `transport/tls.rs`.

### Hand-rolled HTTP/1.1 codec

We don't use `http-body` / `http-body-util`. The codec in `transport/h1.rs`
is ~300 lines and covers the cases we need: `Content-Length`,
`Transfer-Encoding: chunked`, `Connection: close`. Streaming response bodies
are buffered into `Bytes` in the current iteration; the codec returns a
`ResponseHead { body: Bytes }` rather than a streaming body. This is a
deliberate simplification for v1.

### Redirect handling

`SocketsHttpHandler.allow_auto_redirect` defaults to `true`,
`max_redirections` to 5. POST/PUT become GET on 301/302/303 (the body is
dropped). 304 / 305 / 306 are never followed. The redirect loop is in
`handlers/execute.rs::execute_send_with_redirects`.

### Cancellation model

`CancellationToken` wraps `tokio_util::sync::CancellationToken`. The transport
races the actual send against `cancel.cancelled()`. `HttpClient` wraps the
caller's token with its own `with_timeout(self.timeout)` to enforce the
client-level timeout.

`link_with(a, b)` creates a child token that fires when either parent fires.
The implementation spawns two tasks to bridge the parents.

## How to add a new feature

When adding a new `HttpContent` subtype:
1. Add a file in `src/content/`.
2. Implement the `HttpContent` trait — note `&mut self` on `read_as_bytes` /
   `write_to` / `read_as_string`.
3. Add `pub use` in `src/content/mod.rs` and `src/lib.rs`.
4. Add a unit test in the same file.

When adding a new `HttpClient` convenience method:
1. Add it to `HttpClient` in `src/client.rs`. Match the C# shape (verb +
   `Async` suffix, `&self`, takes a `CancellationToken`).
2. If the method takes a body, the body parameter is `Box<dyn HttpContent>`
   (owned) so the caller can pass any subtype.
3. Add an example if it's a common use case.

When adding a new transport feature (compression, etc.):
- Extend `h1.rs` to read/write the relevant headers.
- Extend the request / response flow in `execute.rs`.
- Add an integration test in `tests/integration.rs` that uses the in-process
  server.

## How to run things

```bash
cargo build                 # build the library
cargo test                  # all 38 tests (32 unit + 6 integration)
cargo run --example basic_get
cargo run --example post_string
cargo run --example post_form
cargo clippy --all-targets  # lint check
cargo doc --no-deps         # docs build
```

The integration tests need no external network. The `examples/` scripts hit
`https://httpbin.org/...` and require outbound HTTPS from the host.

## Common pitfalls

- **`response.content()` returns `&dyn HttpContent`** (immutable). For
  `read_as_string` / `read_as_bytes` (which need `&mut self`), use
  **`response.content_mut()`** and declare the binding `mut`.
- **`req.set_content(body)`** takes `Option<Box<dyn HttpContent>>` (note the
  `Some(...)`). The convenience methods (`post_async` etc.) wrap the body
  in `Some` for you.
- **`HttpRequestMessage::content_mut()` returns `&mut (dyn HttpContent +
  'static)`**, not `&mut dyn HttpContent`. This is the variance compromise
  with `Box`. If you need to pass it to a function, use `body.as_deref_mut()`
  or match the explicit `+ 'static` bound.
- **`HeaderName::from_static`** requires lowercase ASCII. The test helper
  in `headers.rs` lowercases for you; raw callers must lowercase first.
