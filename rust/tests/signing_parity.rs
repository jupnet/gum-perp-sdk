use std::num::NonZeroU64;

use gum_perp_sdk::{
    CancelAllOrders, CancelOrder, Hash, MarketParams, MarketSymbol, OrderType, PlaceOrder, Pubkey,
    Side, SignableTradingRequest, TimeInForce,
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

fn signer() -> Pubkey {
    Pubkey::from([7_u8; 32])
}
fn blockhash() -> Hash {
    Hash::from([9_u8; 32])
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn place_order_message_matches_internal_sdk_golden_bytes() {
    let order = PlaceOrder {
        subaccount_id: 1,
        symbol: MarketSymbol("SOL".to_string()),
        side: Side::Bid,
        order_type: OrderType::Limit,
        price: 100.25,
        quantity: 1.5,
        reduce_only: false,
        time_in_force: TimeInForce::GoodTilCanceled,
        client_order_id: Some(NonZeroU64::new(42).unwrap()),
        collateral_amount: None,
    };

    let msg = order
        .message_to_sign(signer(), &TestMarket, blockhash())
        .unwrap();

    assert_eq!(
        hex(&msg),
        "09090909090909090909090909090909090909090909090909090909090909090afd23d1134d8b39513c49c67720abbffa8addf50eb783a890bab5fb6473bedb0107070707070707070707070707070707070707070707070707070707070707072a030103000000534f4cea03000000000000dc050000000000000000000000000000012a00000000000000"
    );
}

#[test]
fn cancel_and_cancel_all_messages_match_internal_sdk_golden_bytes() {
    let cancel = CancelOrder {
        subaccount_id: 2,
        is_isolated: true,
        symbol: MarketSymbol("BTC".to_string()),
        order_id: Some(12345678901234567890_u128),
        client_order_id: Some(NonZeroU64::new(43).unwrap()),
    };
    assert_eq!(
        hex(&cancel.message_to_sign(signer(), &TestMarket, blockhash()).unwrap()),
        "09090909090909090909090909090909090909090909090909090909090909090afd23d1134d8b39513c49c67720abbffa8addf50eb783a890bab5fb6473bedb010707070707070707070707070707070707070707070707070707070707070707231f020300000042544301d20a1feb8ca954ab0000000000000000012b00000000000000"
    );

    let cancel_all = CancelAllOrders {
        subaccount_id: 0,
        is_isolated: false,
        symbol: MarketSymbol("SOL".to_string()),
    };
    assert_eq!(
        hex(&cancel_all.message_to_sign(signer(), &TestMarket, blockhash()).unwrap()),
        "09090909090909090909090909090909090909090909090909090909090909090afd23d1134d8b39513c49c67720abbffa8addf50eb783a890bab5fb6473bedb01070707070707070707070707070707070707070707070707070707070707070709050003000000534f4c"
    );
}
