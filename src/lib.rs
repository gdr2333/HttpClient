//! httpclient — a C#-style `HttpClient` API surface for async Rust.
//!
//! The crate mirrors the public shape of `System.Net.Http.HttpClient` so that
//! callers familiar with the .NET model can map concepts 1:1: `HttpClient`,
//! `HttpRequestMessage`, `HttpResponseMessage`, `HttpMethod`, `HttpHeaders`,
//! `HttpContent` (and its subtypes), `CancellationToken`. Internally the
//! transport is HTTP/1.1 over tokio TCP, with HTTPS provided by rustls.
//!
//! See `client.rs` for the entry point.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod cancellation;
pub mod client;
pub mod content;
pub mod error;
pub mod handlers;
pub mod headers;
pub mod media_type;
pub mod method;
pub mod request;
pub mod response;
pub mod transport;
pub mod uri;
pub mod version;

pub use bytes::Bytes;
pub use cancellation::CancellationToken;
pub use client::{HttpClient, HttpClientBuilder};
pub use content::{
    ByteArrayContent, FormUrlEncodedContent, HttpContent, MultipartContent,
    MultipartFormDataContent, StreamContent, StringContent,
};
pub use error::{HttpRequestError, HttpRequestException, OperationCanceledException};
pub use handlers::{HttpMessageHandler, SocketsHttpHandler};
pub use headers::{HttpContentHeaders, HttpHeaders, HttpRequestHeaders, HttpResponseHeaders};
pub use media_type::MediaTypeHeaderValue;
pub use method::HttpMethod;
pub use request::HttpRequestMessage;
pub use response::HttpResponseMessage;
pub use uri::Uri;
pub use version::{HttpVersion, HttpVersionPolicy};
