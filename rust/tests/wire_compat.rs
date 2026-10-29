use std::num::NonZeroU64;

use gum_perp_sdk::{
    Hash, MarketParams, MarketSymbol, OrderHistoryResponse, OrderStatus, PlaceOrder, Pubkey,
    SignableTradingRequest, TransactionHistoryResponse, TransactionType, TypedSignature,
};

struct TestMarket;

impl MarketParams for TestMarket {
    fn base_decimals(&self) -> u8 {
        9
    }
    fn quote_lot_size(&self) -> u64 {
        100
    }
    fn base_lot_size(&self) -> u64 {
        1_000_000
    }
}

#[test]
fn order_history_decodes_string_client_order_id_and_unknown_status() {
    let row = |status: &str| {
        format!(
            r#"{{"orderId":"1","clientOrderId":"18446744073709551615","symbol":"SOL","side":"Bid",
            "price":"1","originalSize":"1","filledSize":"0.5","realizedPnl":null,"status":"{status}",
            "createdAt":0,"updatedAt":0,"transactions":[],"orderType":"limit","timeInForce":"gtc",
            "reduceOnly":false,"isolated":false,"initialCollateral":null,"executedCollateral":null}}"#
        )
    };
    let body = format!(
        r#"{{"orders":[{},{}]}}"#,
        row("partially_filled_cancelled"),
        row("some_future_status")
    );

    let history: OrderHistoryResponse = serde_json::from_str(&body).unwrap();

    assert_eq!(history.orders[0].client_order_id, Some(u64::MAX));
    assert_eq!(
        history.orders[0].status,
        OrderStatus::PartiallyFilledCancelled
    );
    assert_eq!(history.orders[1].status, OrderStatus::Unknown);
}

#[test]
fn transaction_history_decodes_new_and_unknown_types() {
    let body = r#"{"transactions":[
        {"transactionType":"fee_rebate","amount":"1.5","signature":"s","timestamp":0},
        {"transactionType":"some_future_type","amount":"1","signature":"s","timestamp":0}]}"#;

    let history: TransactionHistoryResponse = serde_json::from_str(body).unwrap();

    assert_eq!(
        history.transactions[0].transaction_type,
        TransactionType::FeeRebate
    );
    assert_eq!(
        history.transactions[1].transaction_type,
        TransactionType::Unknown
    );
}

#[test]
fn isolated_place_order_sends_is_isolated_and_string_client_order_id() {
    let order = PlaceOrder {
        symbol: MarketSymbol("SOL".to_string()),
        price: 100.0,
        quantity: 1.0,
        client_order_id: NonZeroU64::new(42),
        collateral_amount: Some(5_000_000),
        is_isolated: true,
        ..PlaceOrder::default()
    };
    order
        .message_to_sign(Pubkey::from([1; 32]), &TestMarket, Hash::from([2; 32]))
        .unwrap();

    let signed = order.signed_request(
        Pubkey::from([3; 32]),
        Hash::from([2; 32]),
        TypedSignature::from_ed25519([4; 64]),
    );
    let json = serde_json::to_value(&signed.body).unwrap();

    assert_eq!(json["isIsolated"], true);
    assert_eq!(json["collateralAmount"], 5_000_000);
    assert_eq!(json["clientOrderId"], "42");
}
