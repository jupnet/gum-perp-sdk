//! Standalone Rust SDK for market makers/integrators using the Gum Perps API.
//!
//! This crate owns its public wire/signing types so it can be published to
//! crates.io without depending on internal JTX workspace crates.

/// Production Gum Perps API endpoint used by the default constructors.
pub const GUM_API_URL: &str = "https://gum-api.jup.net/jtx/mainnet-beta";

mod action;
mod market;
mod primitives;
mod request;
mod response;
mod rest;
mod types;
mod websocket;
mod ws_types;

pub use {
    action::JTX_PROGRAM_ID,
    market::{MarketInfo, MarketParams, MarketStats, QUOTE_DECIMALS},
    primitives::{
        Hash, ParseBytes32Error, ParseSignatureError, Pubkey, Signer, SignerError, TypedSignature,
    },
    request::{
        ApiSigningError, BatchOrderActionSigned, BatchOrderRequest, BuildTransactionQuery,
        BuildTransferTransactionQuery, CancelAllOrders, CancelAllOrdersSigned, CancelOrder,
        CancelOrderSigned, CreateTradingAccountQuery, FundingHistoryQuery, OhlcvParams,
        OrderHistoryQuery, PlaceOrder, PlaceOrderSigned, RenameSubAccountRequest,
        SignableTradingRequest, SignedApiRequest, SignedCancelAllOrders, SignedCancelOrder,
        SignedPlaceOrder, SubmitTransactionRequest, TradingAccountTradesQuery, TradingAction,
        TransactionHistoryQuery,
    },
    response::*,
    rest::{ApiClientError, ApiClientResult, RestClient},
    types::{Interval, MarketSymbol, OrderType, Side, TimeInForce, ValidationError},
    websocket::{
        IntoTradingActionSigned, WebsocketClient, WebsocketClientError, WebsocketClientResult,
    },
    ws_types::*,
};

#[cfg(feature = "ed25519")]
pub use primitives::keypair::Keypair;
