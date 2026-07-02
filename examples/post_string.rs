//! `post_string` — send a `StringContent` body and read the response.
//!
//! Run with `cargo run --example post_string`.

use httpclient::{HttpClient, CancellationToken, StringContent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let content = StringContent::new("hello from rust httpclient");
    let mut response = client
        .post_async(
            "https://httpbin.org/anything",
            Box::new(content),
            CancellationToken::none(),
        )
        .await?;
    println!("status: {}", response.status_code());
    let body = response.content_mut().read_as_string().await?;
    println!("body:");
    println!("{body}");
    Ok(())
}
