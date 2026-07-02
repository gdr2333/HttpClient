//! `post_form` — send an `application/x-www-form-urlencoded` body and read
//! the response.
//!
//! Run with `cargo run --example post_form`.

use httpclient::{CancellationToken, FormUrlEncodedContent, HttpClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let form = FormUrlEncodedContent::new(vec![("name", "rust"), ("topic", "httpclient")]);
    let mut response = client
        .post_async(
            "https://httpbin.org/post",
            Box::new(form),
            CancellationToken::none(),
        )
        .await?;
    println!("status: {}", response.status_code());
    let body = response.content_mut().read_as_string().await?;
    println!("body:");
    println!("{body}");
    Ok(())
}
