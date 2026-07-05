use gum_perp_sdk::{Pubkey, RestClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Any pubkey works for public data; use your own for account endpoints
    // and trading, where X-PUBKEY must match the signer.
    let user = std::env::var("GUM_PUBKEY")
        .ok()
        .and_then(|s| s.parse::<Pubkey>().ok())
        .unwrap_or_else(Pubkey::new_unique);

    let client = RestClient::new(user);
    let markets = client.markets().await?;
    println!("{} markets", markets.len());
    Ok(())
}
