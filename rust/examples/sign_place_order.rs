use std::num::NonZeroU64;

use gum_perp_sdk::{
    Keypair, MarketSymbol, OrderType, PlaceOrder, RestClient, Side, SignableTradingRequest,
    TimeInForce,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer = Keypair::new();
    let user = signer.pubkey();
    let client = RestClient::new(user);

    let market = client.market("SOL").await?;
    let blockhash = client.blockhash().await?.blockhash;
    let order = PlaceOrder {
        subaccount_id: 0,
        symbol: MarketSymbol("SOL".to_string()),
        side: Side::Bid,
        order_type: OrderType::Limit,
        price: 100.0,
        quantity: 1.0,
        reduce_only: false,
        time_in_force: TimeInForce::GoodTilCanceled,
        client_order_id: Some(NonZeroU64::new(1).unwrap()),
        collateral_amount: None,
    };

    let signed = order.sign_request_with(user, &market, blockhash, &signer)?;
    println!("{} {}", signed.method, signed.path);
    // let response = client.place_order(&signed).await?;
    Ok(())
}
