use gum_perp_sdk::{OrderHistoryResponse, OrderStatus};

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
