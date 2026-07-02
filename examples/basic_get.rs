//! `basic_get` — the smallest possible example. Sends a `GET` to
//! `https://httpbin.org/get` and prints the response status + first 200
//! bytes of the body.
//!
//! Run with `cargo run --example basic_get`.

use httpclient::{CancellationToken, HttpClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let mut response = client
        .get_async("https://bing.com", CancellationToken::none())
        .await?;
    println!("status: {}", response.status_code());
    let mut body = response.content_mut().read_as_bytes().await?;
    if body.len() > 200 {
        body.truncate(200);
    }
    println!("body (first 200 bytes):");
    println!("{}", String::from_utf8_lossy(&body));
    Ok(())
}
