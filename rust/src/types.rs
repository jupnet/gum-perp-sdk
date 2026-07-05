use {
    borsh::{BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
    std::num::NonZeroU32,
    thiserror::Error,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MarketSymbol(pub String);

impl std::fmt::Display for MarketSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<&MarketSymbol> for [u8; 32] {
    type Error = ValidationError;

    fn try_from(symbol: &MarketSymbol) -> Result<Self, Self::Error> {
        let bytes = symbol.0.as_bytes();
        if bytes.len() > 32 {
            return Err(ValidationError::SymbolTooLarge);
        }
        let mut array = [0_u8; 32];
        array[..bytes.len()].copy_from_slice(bytes);
        Ok(array)
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Market symbol too large")]
    SymbolTooLarge,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

#[repr(u8)]
#[derive(
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    PartialEq,
    Serialize,
)]
pub enum Side {
    #[default]
    #[serde(alias = "bid", alias = "buy", alias = "Buy")]
    Bid,
    #[serde(alias = "ask", alias = "sell", alias = "Sell")]
    Ask,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Bid => Self::Ask,
            Self::Ask => Self::Bid,
        }
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bid => write!(f, "bid"),
            Self::Ask => write!(f, "ask"),
        }
    }
}

#[repr(u8)]
#[derive(
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    Eq,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
pub enum OrderType {
    #[default]
    Limit,
    Market,
    PostOnly,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit => write!(f, "limit"),
            Self::Market => write!(f, "market"),
            Self::PostOnly => write!(f, "postOnly"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum TimeInForce {
    #[default]
    #[serde(alias = "GTC")]
    GoodTilCanceled,
    #[serde(alias = "IOC")]
    ImmediateOrCancel,
    #[serde(alias = "FOK")]
    FillOrKill,
    Seconds(NonZeroU32),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, Debug, PartialEq)]
pub enum OnchainTimeInForce {
    Seconds(u32),
    ImmediateOrCancel,
    FillOrKill,
}

impl Default for OnchainTimeInForce {
    fn default() -> Self {
        Self::Seconds(0)
    }
}

impl From<TimeInForce> for OnchainTimeInForce {
    fn from(tif: TimeInForce) -> Self {
        match tif {
            TimeInForce::GoodTilCanceled => Self::Seconds(0),
            TimeInForce::ImmediateOrCancel => Self::ImmediateOrCancel,
            TimeInForce::FillOrKill => Self::FillOrKill,
            TimeInForce::Seconds(secs) => Self::Seconds(secs.get()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    #[serde(rename = "1m", alias = "1M")]
    M1,
    #[serde(rename = "5m", alias = "5M")]
    M5,
    #[serde(rename = "15m", alias = "15M")]
    M15,
    #[serde(rename = "1h", alias = "1H")]
    H1,
    #[serde(rename = "4h", alias = "4H")]
    H4,
    #[serde(rename = "1d", alias = "1D")]
    D1,
}

impl Interval {
    pub const ALL: [Interval; 6] = [Self::M1, Self::M5, Self::M15, Self::H1, Self::H4, Self::D1];

    pub fn seconds(&self) -> i64 {
        match self {
            Self::M1 => 60,
            Self::M5 => 300,
            Self::M15 => 900,
            Self::H1 => 3600,
            Self::H4 => 14400,
            Self::D1 => 86400,
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::M1 => write!(f, "1m"),
            Self::M5 => write!(f, "5m"),
            Self::M15 => write!(f, "15m"),
            Self::H1 => write!(f, "1h"),
            Self::H4 => write!(f, "4h"),
            Self::D1 => write!(f, "1d"),
        }
    }
}
