use gum_perp_sdk::WebsocketClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = WebsocketClient::connect().await?;
    ws.subscribe_blockhash().await?;
    ws.ping("ping-1").await?;
    Ok(())
}
