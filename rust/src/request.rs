use {
    crate::{
        action::{
            serialize_action_message, CancelAllOrdersParams, CancelOrderParams,
            PlaceOrderIsolatedParams, PlaceOrderParams, TradingParams,
        },
        types::OnchainTimeInForce,
        Hash, MarketParams, MarketSymbol, OrderStatus, OrderType, Pubkey, Side, Signer,
        SignerError, TimeInForce, TransactionType, TypedSignature, ValidationError,
    },
    serde::{Deserialize, Serialize},
    std::num::NonZeroU64,
    thiserror::Error,
};

pub trait JTXAction {
    fn symbol(&self) -> &MarketSymbol;
    fn params(&self, market: &dyn MarketParams) -> Result<TradingParams, ValidationError>;

    fn message_to_sign(
        &self,
        signer: Pubkey,
        market: &dyn MarketParams,
        recent_blockhash: Hash,
    ) -> Result<Vec<u8>, ValidationError> {
        let data = self.params(market)?.data();
        Ok(serialize_action_message(signer, recent_blockhash, &data))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SignedApiRequest<T> {
    pub method: &'static str,
    pub path: &'static str,
    pub body: T,
}

#[derive(Debug, Error)]
pub enum ApiSigningError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("failed to read signer pubkey: {0}")]
    SignerPubkey(#[source] SignerError),
    #[error("failed to sign JTX action: {0}")]
    Sign(#[source] SignerError),
}

pub trait SignableTradingRequest: JTXAction + Clone + Sized {
    type SignedBody: Serialize;

    const PATH: &'static str;

    fn into_signed_body(
        self,
        trading_account: Pubkey,
        recent_blockhash: Hash,
        signature: TypedSignature,
    ) -> Self::SignedBody;

    fn message_to_sign(
        &self,
        signer: Pubkey,
        market: &dyn MarketParams,
        recent_blockhash: Hash,
    ) -> Result<Vec<u8>, ApiSigningError> {
        <Self as JTXAction>::message_to_sign(self, signer, market, recent_blockhash)
            .map_err(ApiSigningError::Validation)
    }

    fn sign_with<S: Signer + ?Sized>(
        &self,
        market: &dyn MarketParams,
        recent_blockhash: Hash,
        signer: &S,
    ) -> Result<TypedSignature, ApiSigningError> {
        let signer_pubkey = signer.try_pubkey().map_err(ApiSigningError::SignerPubkey)?;
        let message =
            SignableTradingRequest::message_to_sign(self, signer_pubkey, market, recent_blockhash)?;
        signer
            .try_sign_message(&message)
            .map_err(ApiSigningError::Sign)
    }

    fn signed_request(
        self,
        trading_account: Pubkey,
        recent_blockhash: Hash,
        signature: TypedSignature,
    ) -> SignedApiRequest<Self::SignedBody> {
        SignedApiRequest {
            method: "POST",
            path: Self::PATH,
            body: self.into_signed_body(trading_account, recent_blockhash, signature),
        }
    }

    fn sign_request_with<S: Signer + ?Sized>(
        self,
        trading_account: Pubkey,
        market: &dyn MarketParams,
        recent_blockhash: Hash,
        signer: &S,
    ) -> Result<SignedApiRequest<Self::SignedBody>, ApiSigningError> {
        let signature = self.sign_with(market, recent_blockhash, signer)?;
        Ok(self.signed_request(trading_account, recent_blockhash, signature))
    }
}

macro_rules! signed {
    ($signed:ident, $order:ident) => {
        #[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        pub struct $signed {
            #[serde(flatten)]
            pub order: $order,
            pub trading_account: Pubkey,
            pub recent_blockhash: Hash,
            pub signature: TypedSignature,
        }
    };
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TradingAction {
    Place(PlaceOrder),
    Cancel(CancelOrder),
    CancelAll(CancelAllOrders),
}

signed!(PlaceOrderSigned, PlaceOrder);
signed!(CancelOrderSigned, CancelOrder);
signed!(CancelAllOrdersSigned, CancelAllOrders);

pub type SignedPlaceOrder = PlaceOrderSigned;
pub type SignedCancelOrder = CancelOrderSigned;
pub type SignedCancelAllOrders = CancelAllOrdersSigned;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PlaceOrder {
    #[serde(default)]
    pub subaccount_id: u8,
    pub symbol: MarketSymbol,
    pub side: Side,
    #[serde(default)]
    pub order_type: OrderType,
    #[serde(default)]
    pub price: f64,
    pub quantity: f64,
    #[serde(default)]
    pub reduce_only: bool,
    #[serde(default)]
    pub time_in_force: TimeInForce,
    pub client_order_id: Option<NonZeroU64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral_amount: Option<u64>,
}

impl JTXAction for PlaceOrder {
    fn symbol(&self) -> &MarketSymbol {
        &self.symbol
    }

    fn params(&self, market: &dyn MarketParams) -> Result<TradingParams, ValidationError> {
        let (price_lots, base_lots, time_in_force) = match self.order_type {
            OrderType::Limit | OrderType::PostOnly => {
                if self.price <= 0.0 {
                    return Err(ValidationError::InvalidInput(
                        "Limit and post-only orders require a positive price".to_string(),
                    ));
                }
                if self.order_type == OrderType::PostOnly
                    && matches!(
                        self.time_in_force,
                        TimeInForce::ImmediateOrCancel | TimeInForce::FillOrKill
                    )
                {
                    return Err(ValidationError::InvalidInput(
                        "Post-only orders cannot use IOC or FOK time-in-force".to_string(),
                    ));
                }
                let (price_lots, base_lots) = market.to_lots(self.price, self.quantity)?;
                (price_lots, base_lots, self.time_in_force.into())
            }
            OrderType::Market => {
                let tif = match self.time_in_force {
                    TimeInForce::GoodTilCanceled | TimeInForce::ImmediateOrCancel => {
                        OnchainTimeInForce::ImmediateOrCancel
                    }
                    TimeInForce::FillOrKill => OnchainTimeInForce::FillOrKill,
                    TimeInForce::Seconds(_) => {
                        return Err(ValidationError::InvalidInput(
                            "Market orders only support IOC or FOK time-in-force".to_string(),
                        ))
                    }
                };
                let base_lots = market.to_base_lots(self.quantity)?;
                let price_lots = if self.price > 0.0 {
                    let (pl, _) = market.to_lots(self.price, self.quantity)?;
                    pl
                } else {
                    0
                };
                (price_lots, base_lots, tif)
            }
        };

        let order = PlaceOrderParams {
            subaccount_id: self.subaccount_id,
            symbol: self.symbol.to_string(),
            price_lots,
            base_lots,
            side: self.side,
            order_type: self.order_type,
            reduce_only: self.reduce_only,
            time_in_force,
            client_order_id: self.client_order_id,
        };

        Ok(if let Some(collateral_amount) = self.collateral_amount {
            TradingParams::PlaceIsolated(PlaceOrderIsolatedParams {
                collateral_amount,
                order,
            })
        } else {
            TradingParams::Place(order)
        })
    }
}

impl SignableTradingRequest for PlaceOrder {
    type SignedBody = PlaceOrderSigned;
    const PATH: &'static str = "/order/create";

    fn into_signed_body(
        self,
        trading_account: Pubkey,
        recent_blockhash: Hash,
        signature: TypedSignature,
    ) -> Self::SignedBody {
        PlaceOrderSigned {
            order: self,
            trading_account,
            recent_blockhash,
            signature,
        }
    }
}

#[serde_with::serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CancelOrder {
    #[serde(default)]
    pub subaccount_id: u8,
    #[serde(default)]
    pub is_isolated: bool,
    pub symbol: MarketSymbol,
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<u128>,
    pub client_order_id: Option<NonZeroU64>,
}

impl JTXAction for CancelOrder {
    fn symbol(&self) -> &MarketSymbol {
        &self.symbol
    }

    fn params(&self, _market: &dyn MarketParams) -> Result<TradingParams, ValidationError> {
        let params = CancelOrderParams {
            subaccount_id: self.subaccount_id,
            symbol: self.symbol.to_string(),
            order_id: self.order_id,
            client_order_id: self.client_order_id,
        };
        Ok(if self.is_isolated {
            TradingParams::CancelIsolated(params)
        } else {
            TradingParams::Cancel(params)
        })
    }
}

impl SignableTradingRequest for CancelOrder {
    type SignedBody = CancelOrderSigned;
    const PATH: &'static str = "/order/cancel";

    fn into_signed_body(
        self,
        trading_account: Pubkey,
        recent_blockhash: Hash,
        signature: TypedSignature,
    ) -> Self::SignedBody {
        CancelOrderSigned {
            order: self,
            trading_account,
            recent_blockhash,
            signature,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CancelAllOrders {
    #[serde(default)]
    pub subaccount_id: u8,
    #[serde(default)]
    pub is_isolated: bool,
    pub symbol: MarketSymbol,
}

impl JTXAction for CancelAllOrders {
    fn symbol(&self) -> &MarketSymbol {
        &self.symbol
    }

    fn params(&self, _market: &dyn MarketParams) -> Result<TradingParams, ValidationError> {
        let params = CancelAllOrdersParams {
            subaccount_id: self.subaccount_id,
            symbol: self.symbol.to_string(),
        };
        Ok(if self.is_isolated {
            TradingParams::CancelAllIsolated(params)
        } else {
            TradingParams::CancelAll(params)
        })
    }
}

impl SignableTradingRequest for CancelAllOrders {
    type SignedBody = CancelAllOrdersSigned;
    const PATH: &'static str = "/order/cancel-all";

    fn into_signed_body(
        self,
        trading_account: Pubkey,
        recent_blockhash: Hash,
        signature: TypedSignature,
    ) -> Self::SignedBody {
        CancelAllOrdersSigned {
            order: self,
            trading_account,
            recent_blockhash,
            signature,
        }
    }
}

/// Signed action item accepted by `POST /order/batch`.
///
/// Batch v1 intentionally supports only signed `place` and `cancel`; use
/// `POST /order/cancel-all` separately for cancel-all requests.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum BatchOrderActionSigned {
    Place(PlaceOrderSigned),
    Cancel(CancelOrderSigned),
}

impl BatchOrderActionSigned {
    pub fn symbol(&self) -> &MarketSymbol {
        match self {
            Self::Place(signed) => &signed.order.symbol,
            Self::Cancel(signed) => &signed.order.symbol,
        }
    }
}

impl From<PlaceOrderSigned> for BatchOrderActionSigned {
    fn from(signed: PlaceOrderSigned) -> Self {
        Self::Place(signed)
    }
}

impl From<CancelOrderSigned> for BatchOrderActionSigned {
    fn from(signed: CancelOrderSigned) -> Self {
        Self::Cancel(signed)
    }
}

/// Request body for `POST /order/batch`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BatchOrderRequest {
    pub actions: Vec<BatchOrderActionSigned>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildTransactionQuery {
    pub amount: u64,
    pub trading_account: String,
}

/// GET /transfer - Query parameters
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildTransferTransactionQuery {
    pub amount: u64,
    pub source_trading_account: String,
    pub destination_trading_account: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CreateTradingAccountQuery {
    #[serde(default)]
    pub is_isolated: bool,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub subaccount_id: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTransactionRequest {
    pub serialized_transaction: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenameSubAccountRequest {
    pub name: String,
    pub blockhash: Hash,
    pub signature: TypedSignature,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OhlcvParams {
    pub interval: crate::Interval,
    pub from: i64,
    pub to: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingAccountTradesQuery {
    pub symbol: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderHistoryQuery {
    pub symbol: Option<String>,
    pub status: Option<OrderStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionHistoryQuery {
    pub transaction_type: Option<TransactionType>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingHistoryQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[cfg(test)]
mod tests {
    use {
        super::{CancelOrder, PlaceOrder},
        crate::{MarketSymbol, TimeInForce},
        std::num::NonZeroU32,
    };

    // The API's own `TimeInForce` derives Deserialize with the same shape
    // (variant names as strings, `Seconds` externally tagged; "GTC"/"IOC"/"FOK"
    // are input aliases only) — docs/REST_API.md documents "GoodTilCanceled" as
    // the canonical wire value. Pin the emitted forms.
    #[test]
    fn time_in_force_serializes_to_the_documented_wire_forms() {
        let mut order = PlaceOrder {
            symbol: MarketSymbol("SOL".to_string()),
            quantity: 1.0,
            ..PlaceOrder::default()
        };

        let json = serde_json::to_value(&order).unwrap();
        assert_eq!(json["timeInForce"], "GoodTilCanceled");

        order.time_in_force = TimeInForce::Seconds(NonZeroU32::new(30).unwrap());
        let json = serde_json::to_value(&order).unwrap();
        assert_eq!(json["timeInForce"]["Seconds"], 30);

        let gtc: TimeInForce = serde_json::from_str("\"GTC\"").unwrap();
        assert!(matches!(gtc, TimeInForce::GoodTilCanceled));
    }

    #[test]
    fn cancel_order_id_serializes_as_string_and_omits_none() {
        let mut cancel = CancelOrder {
            order_id: Some(12345678901234567890_u128),
            ..CancelOrder::default()
        };

        let json = serde_json::to_value(&cancel).unwrap();
        assert_eq!(json["orderId"], "12345678901234567890");

        let roundtrip: CancelOrder = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, cancel);

        cancel.order_id = None;
        let json = serde_json::to_value(&cancel).unwrap();
        assert!(json.get("orderId").is_none());
    }
}
