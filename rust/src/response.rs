use {
    crate::{Hash, Side},
    serde::{Deserialize, Serialize},
};

pub type BookEntry = [f64; 2];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookResponse {
    pub symbol: String,
    pub bids: Vec<BookEntry>,
    pub asks: Vec<BookEntry>,
    pub timestamp: u64,
    pub last_updated_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockhashResponse {
    pub blockhash: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTradingAccountResponse {
    pub trading_account: String,
    pub subaccount_id: u8,
    pub transaction_signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameSubAccountResponse {
    pub pubkey: String,
    pub subaccount_id: u8,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAccountData {
    pub free_balance: String,
    pub token_balance: u64,
    #[serde(default)]
    pub account_value: Option<String>,
    #[serde(default)]
    pub total_maintenance_margin: Option<String>,
    #[serde(default)]
    pub total_liquidation_requirement: Option<String>,
    #[serde(default)]
    pub cross_account_leverage: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAccountBalancesResponse {
    pub free_balance: String,
    pub token_balance: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpPositionResponse {
    pub symbol: String,
    pub side: Side,
    pub size: String,
    pub avg_entry_price: String,
    pub mark_price: String,
    pub unrealized_pnl: String,
    pub unrealized_pnl_percent: String,
    pub margin: String,
    pub liquidation_price: Option<String>,
    pub funding_accrued: String,
    pub pending_funding_payable: String,
    pub pending_funding_receivable: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckTradingAccountResponse {
    pub exists: bool,
    pub accounts: Vec<SubAccount>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAccount {
    pub pubkey: String,
    pub subaccount_id: u8,
    pub name: Option<String>,
    pub data: TradingAccountData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildTransactionResponse {
    pub serialized_transaction: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitTransactionResponse {
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceResponse {
    pub symbol: String,
    pub client_order_id: Option<std::num::NonZeroU64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelResponse {
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllResponse {
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOrderResponse {
    pub success: bool,
    pub accepted_actions: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeResponse {
    pub symbol: String,
    pub side: Side,
    pub price: String,
    pub size: String,
    pub txid: String,
    pub time: i64,
    pub trading_accounts: [String; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAccountTradeResponse {
    pub symbol: String,
    pub side: Side,
    pub price: String,
    pub size: String,
    pub txid: String,
    pub time: i64,
    pub client_order_id: Option<u64>,
    pub fee: String,
    pub realized_pnl: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradesResponse {
    pub symbol: String,
    pub trades: Vec<TradeResponse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

impl OrderStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl std::str::FromStr for OrderStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "partially_filled" => Ok(Self::PartiallyFilled),
            "filled" => Ok(Self::Filled),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid order status '{s}'")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderHistoryResponse {
    pub orders: Vec<OrderHistoryEntry>,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderHistoryEntry {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub order_id: u128,
    pub client_order_id: Option<u64>,
    pub symbol: String,
    pub side: Side,
    pub price: String,
    pub original_size: String,
    pub filled_size: String,
    pub realized_pnl: Option<String>,
    pub status: OrderStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub transactions: Vec<OrderTransaction>,
    pub order_type: String,
    pub time_in_force: String,
    pub reduce_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderTransaction {
    pub signature: String,
    #[serde(rename = "type")]
    pub tx_type: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation_fee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdraw,
    SettlePnl,
    SettleFunding,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deposit => write!(f, "deposit"),
            Self::Withdraw => write!(f, "withdraw"),
            Self::SettlePnl => write!(f, "settle_pnl"),
            Self::SettleFunding => write!(f, "settle_funding"),
        }
    }
}

impl std::str::FromStr for TransactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deposit" => Ok(Self::Deposit),
            "withdraw" => Ok(Self::Withdraw),
            "settle_pnl" => Ok(Self::SettlePnl),
            "settle_funding" => Ok(Self::SettleFunding),
            _ => Err(format!("Invalid transaction type '{s}'")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionHistoryResponse {
    pub transactions: Vec<TransactionHistoryEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionHistoryEntry {
    pub transaction_type: TransactionType,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRateResponse {
    pub symbol: String,
    pub rate_bps_1h: f64,
    pub funding_rate: String,
    pub predicted_funding_rate: String,
    pub funding_interval_seconds: u64,
    pub next_funding_collection_time: u64,
    pub next_funding_distribution_time: u64,
    pub funding_update_timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRatesResponse {
    pub funding_rates: Vec<FundingRateResponse>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRateHistoryEntry {
    pub symbol: String,
    pub rate_bps_1h: f64,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRateHistoryResponse {
    pub entries: Vec<FundingRateHistoryEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFundingEntry {
    pub symbol: String,
    pub funding: f64,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFundingResponse {
    pub entries: Vec<UserFundingEntry>,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrderResponse {
    pub symbol: String,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub order_id: u128,
    pub side: Side,
    pub price: String,
    pub initial_size: String,
    pub remaining_size: String,
    pub expiry_timestamp: u64,
    pub status: OrderStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<u64>,
    pub reduce_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CandleResponse {
    #[serde(rename = "t")]
    pub open_time: i64,
    #[serde(rename = "T")]
    pub close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: f64,
    #[serde(rename = "c")]
    pub close: f64,
    #[serde(rename = "h")]
    pub high: f64,
    #[serde(rename = "l")]
    pub low: f64,
    #[serde(rename = "v")]
    pub volume: f64,
    #[serde(rename = "n")]
    pub trade_count: u64,
}
