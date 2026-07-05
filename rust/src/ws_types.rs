use {
    crate::{
        CancelAllOrdersSigned, CancelOrderSigned, CandleResponse, Interval, OpenOrderResponse,
        OrderbookResponse, PerpPositionResponse, PlaceOrderSigned, Pubkey, TradeResponse,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum TradingActionSigned {
    Place(PlaceOrderSigned),
    Cancel(CancelOrderSigned),
    CancelAll(CancelAllOrdersSigned),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum ClientMessage {
    Subscribe { subscription: Subscription },
    Unsubscribe { subscription: Subscription },
    Post(Box<PostRequest>),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Subscription {
    Blockhash,
    TradingAccountEvents {
        pubkey: Pubkey,
    },
    Positions {
        pubkey: Pubkey,
    },
    OpenOrders {
        pubkey: Pubkey,
    },
    Trades {
        symbol: String,
    },
    Orderbook {
        symbol: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "updateMs")]
        update_ms: Option<u64>,
    },
    Candle {
        symbol: String,
        interval: Interval,
    },
    MarketStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CandleKey {
    pub symbol: String,
    pub interval: Interval,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostRequest {
    pub id: String,
    pub payload: PostPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostPayload {
    pub pubkey: Pubkey,
    pub action: TradingActionSigned,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum WebsocketResponse {
    Blockhash(crate::BlockhashResponse),
    TradingAccountEvents(serde_json::Value),
    Positions(PerpPositionResponse),
    OpenOrders(OpenOrderResponse),
    Trades(TradeResponse),
    Orderbook(OrderbookResponse),
    Candle(CandleResponse),
    MarketStats(MarketStatsTickResponse),
    Subscription(SubscriptionResponse),
    Request(RequestResponse),
    Error(ErrorResponse),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatsTick {
    pub symbol: String,
    pub index_price: f64,
    pub mark_price: f64,
    pub open_interest: f64,
    pub open_interest_base: f64,
    pub funding_rate_bps_1h: Option<f64>,
    pub price_change_24h: f64,
    pub price_change_percent_24h: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketStatsTickResponse {
    pub timestamp: u64,
    pub markets: Vec<MarketStatsTick>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionMethod {
    Subscribe,
    Unsubscribe,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "subscriptionResponse", rename_all = "camelCase")]
pub struct SubscriptionResponse {
    pub method: SubscriptionMethod,
    pub subscription: Subscription,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "requestResponse", rename_all = "camelCase")]
pub struct RequestResponse {
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub message: String,
}
