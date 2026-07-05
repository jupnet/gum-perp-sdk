use {
    crate::{
        ApiSigningError, BatchOrderRequest, BatchOrderResponse, BlockhashResponse,
        BuildTransactionQuery, BuildTransactionResponse, BuildTransferTransactionQuery,
        CancelAllOrdersSigned, CancelAllResponse, CancelOrderSigned, CancelResponse,
        CandleResponse, CheckTradingAccountResponse, CreateTradingAccountQuery,
        CreateTradingAccountResponse, FundingHistoryQuery, FundingRateHistoryResponse,
        FundingRateResponse, FundingRatesResponse, MarketInfo, OhlcvParams, OpenOrderResponse,
        OrderHistoryQuery, OrderHistoryResponse, OrderbookResponse, PerpPositionResponse,
        PlaceOrderSigned, PlaceResponse, Pubkey, RenameSubAccountRequest, RenameSubAccountResponse,
        SignedApiRequest, SubmitTransactionRequest, SubmitTransactionResponse, TradesResponse,
        TradingAccountBalancesResponse, TradingAccountTradeResponse, TradingAccountTradesQuery,
        TransactionHistoryQuery, TransactionHistoryResponse, UserFundingResponse,
    },
    reqwest::{Client as HttpClient, Method, StatusCode},
    serde::{de::DeserializeOwned, Serialize},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ApiClientError {
    #[error("invalid API base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API returned {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("failed to decode API response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error(transparent)]
    Signing(#[from] ApiSigningError),
}

pub type ApiClientResult<T> = Result<T, ApiClientError>;

#[derive(Clone, Debug)]
pub struct RestClient {
    base_url: String,
    pubkey: Pubkey,
    http: HttpClient,
}

impl RestClient {
    /// Client for the production Gum Perps API ([`crate::GUM_API_URL`]).
    ///
    /// `pubkey` identifies the user: it is sent as the `X-PUBKEY` header on
    /// every request and must match the signer of trading actions.
    pub fn new(pubkey: Pubkey) -> Self {
        Self::with_base_url(crate::GUM_API_URL, pubkey).expect("GUM_API_URL is a valid base URL")
    }

    /// Client for a custom API endpoint (staging, local development).
    pub fn with_base_url(base_url: impl Into<String>, pubkey: Pubkey) -> ApiClientResult<Self> {
        Self::with_http_client(base_url, pubkey, HttpClient::new())
    }

    pub fn with_http_client(
        base_url: impl Into<String>,
        pubkey: Pubkey,
        http: HttpClient,
    ) -> ApiClientResult<Self> {
        let base_url = normalize_base_url(base_url.into())?;
        Ok(Self {
            base_url,
            pubkey,
            http,
        })
    }

    pub fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    pub fn endpoint_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.endpoint_url(path))
            .header("X-PUBKEY", self.pubkey.to_string())
    }

    async fn parse_json<T: DeserializeOwned>(response: reqwest::Response) -> ApiClientResult<T> {
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(ApiClientError::Status {
                status,
                body: String::from_utf8_lossy(&bytes).to_string(),
            });
        }
        serde_json::from_slice(&bytes).map_err(ApiClientError::Decode)
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> ApiClientResult<T> {
        let response = self.request(Method::GET, path).send().await?;
        Self::parse_json(response).await
    }

    pub async fn get_json_with_query<T, Q>(&self, path: &str, query: &Q) -> ApiClientResult<T>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let response = self.request(Method::GET, path).query(query).send().await?;
        Self::parse_json(response).await
    }

    pub async fn post_json<T, B>(&self, path: &str, body: &B) -> ApiClientResult<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let response = self.request(Method::POST, path).json(body).send().await?;
        Self::parse_json(response).await
    }

    pub async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> ApiClientResult<T> {
        let response = self.request(Method::POST, path).send().await?;
        Self::parse_json(response).await
    }

    pub async fn post_empty_with_query<T, Q>(&self, path: &str, query: &Q) -> ApiClientResult<T>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let response = self.request(Method::POST, path).query(query).send().await?;
        Self::parse_json(response).await
    }

    pub async fn post_signed<T, B>(&self, request: &SignedApiRequest<B>) -> ApiClientResult<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        self.post_json(request.path, &request.body).await
    }

    pub async fn markets(&self) -> ApiClientResult<Vec<MarketInfo>> {
        self.get_json("/markets").await
    }
    pub async fn market(&self, symbol: &str) -> ApiClientResult<MarketInfo> {
        self.get_json(&format!("/market/{symbol}")).await
    }

    pub async fn orderbook(
        &self,
        symbol: &str,
        depth: Option<usize>,
    ) -> ApiClientResult<OrderbookResponse> {
        #[derive(Serialize)]
        struct Query {
            depth: Option<usize>,
        }
        self.get_json_with_query(&format!("/orderbook/{symbol}"), &Query { depth })
            .await
    }

    pub async fn trades(&self, symbol: &str) -> ApiClientResult<TradesResponse> {
        self.get_json(&format!("/trades/{symbol}")).await
    }
    pub async fn ohlcv(
        &self,
        symbol: &str,
        params: &OhlcvParams,
    ) -> ApiClientResult<Vec<CandleResponse>> {
        self.get_json_with_query(&format!("/ohlcv/{symbol}"), params)
            .await
    }
    pub async fn funding_rates(&self) -> ApiClientResult<FundingRatesResponse> {
        self.get_json("/fundingRates").await
    }
    pub async fn funding_rate(&self, symbol: &str) -> ApiClientResult<FundingRateResponse> {
        self.get_json(&format!("/fundingRate/{symbol}")).await
    }
    pub async fn funding_rate_history(
        &self,
        symbol: &str,
        query: &FundingHistoryQuery,
    ) -> ApiClientResult<FundingRateHistoryResponse> {
        self.get_json_with_query(&format!("/fundingRate/{symbol}/history"), query)
            .await
    }
    pub async fn user_funding_history(
        &self,
        trading_account: &Pubkey,
        query: &FundingHistoryQuery,
    ) -> ApiClientResult<UserFundingResponse> {
        self.get_json_with_query(&format!("/tradingAccount/{trading_account}/funding"), query)
            .await
    }
    pub async fn blockhash(&self) -> ApiClientResult<BlockhashResponse> {
        self.get_json("/blockhash").await
    }
    pub async fn create_trading_account(
        &self,
        query: &CreateTradingAccountQuery,
    ) -> ApiClientResult<CreateTradingAccountResponse> {
        self.post_empty_with_query("/tradingAccount", query).await
    }
    pub async fn rename_subaccount(
        &self,
        trading_account: &Pubkey,
        request: &RenameSubAccountRequest,
    ) -> ApiClientResult<RenameSubAccountResponse> {
        self.post_json(&format!("/tradingAccount/{trading_account}/name"), request)
            .await
    }
    pub async fn trading_accounts(&self) -> ApiClientResult<CheckTradingAccountResponse> {
        self.get_json("/tradingAccount").await
    }
    pub async fn open_orders(
        &self,
        trading_account: &Pubkey,
    ) -> ApiClientResult<Vec<OpenOrderResponse>> {
        self.get_json(&format!("/tradingAccount/{trading_account}/orders"))
            .await
    }
    pub async fn positions(
        &self,
        trading_account: &Pubkey,
    ) -> ApiClientResult<Vec<PerpPositionResponse>> {
        self.get_json(&format!("/tradingAccount/{trading_account}/positions"))
            .await
    }
    pub async fn balances(
        &self,
        trading_account: &Pubkey,
    ) -> ApiClientResult<TradingAccountBalancesResponse> {
        self.get_json(&format!("/tradingAccount/{trading_account}/balances"))
            .await
    }
    pub async fn trading_account_trades(
        &self,
        trading_account: &Pubkey,
        query: &TradingAccountTradesQuery,
    ) -> ApiClientResult<Vec<TradingAccountTradeResponse>> {
        self.get_json_with_query(&format!("/tradingAccount/{trading_account}/trades"), query)
            .await
    }
    pub async fn order_history(
        &self,
        trading_account: &Pubkey,
        query: &OrderHistoryQuery,
    ) -> ApiClientResult<OrderHistoryResponse> {
        self.get_json_with_query(
            &format!("/tradingAccount/{trading_account}/orderHistory"),
            query,
        )
        .await
    }
    pub async fn transaction_history(
        &self,
        trading_account: &Pubkey,
        query: &TransactionHistoryQuery,
    ) -> ApiClientResult<TransactionHistoryResponse> {
        self.get_json_with_query(
            &format!("/tradingAccount/{trading_account}/transactionHistory"),
            query,
        )
        .await
    }
    pub async fn build_deposit_transaction(
        &self,
        amount: u64,
        trading_account: &Pubkey,
    ) -> ApiClientResult<BuildTransactionResponse> {
        self.get_json_with_query(
            "/deposit",
            &BuildTransactionQuery {
                amount,
                trading_account: trading_account.to_string(),
            },
        )
        .await
    }
    pub async fn submit_deposit(
        &self,
        serialized_transaction: impl Into<String>,
    ) -> ApiClientResult<SubmitTransactionResponse> {
        self.post_json(
            "/deposit",
            &SubmitTransactionRequest {
                serialized_transaction: serialized_transaction.into(),
            },
        )
        .await
    }
    pub async fn build_withdraw_transaction(
        &self,
        amount: u64,
        trading_account: &Pubkey,
    ) -> ApiClientResult<BuildTransactionResponse> {
        self.get_json_with_query(
            "/withdraw",
            &BuildTransactionQuery {
                amount,
                trading_account: trading_account.to_string(),
            },
        )
        .await
    }
    pub async fn submit_withdraw(
        &self,
        serialized_transaction: impl Into<String>,
    ) -> ApiClientResult<SubmitTransactionResponse> {
        self.post_json(
            "/withdraw",
            &SubmitTransactionRequest {
                serialized_transaction: serialized_transaction.into(),
            },
        )
        .await
    }

    /// Build an unsigned collateral transfer between two trading accounts
    /// owned by this client's pubkey (cross<->cross subaccounts, or a
    /// cross-margin subaccount and one of its isolated accounts; the
    /// direction is inferred from the account types).
    pub async fn build_transfer_transaction(
        &self,
        amount: u64,
        source_trading_account: &Pubkey,
        destination_trading_account: &Pubkey,
    ) -> ApiClientResult<BuildTransactionResponse> {
        self.get_json_with_query(
            "/transfer",
            &BuildTransferTransactionQuery {
                amount,
                source_trading_account: source_trading_account.to_string(),
                destination_trading_account: destination_trading_account.to_string(),
            },
        )
        .await
    }

    /// Submit a user-signed collateral transfer transaction; the server
    /// co-signs as fee payer.
    pub async fn submit_transfer(
        &self,
        serialized_transaction: impl Into<String>,
    ) -> ApiClientResult<SubmitTransactionResponse> {
        self.post_json(
            "/transfer",
            &SubmitTransactionRequest {
                serialized_transaction: serialized_transaction.into(),
            },
        )
        .await
    }

    pub async fn place_order(
        &self,
        request: &SignedApiRequest<PlaceOrderSigned>,
    ) -> ApiClientResult<PlaceResponse> {
        self.post_signed(request).await
    }
    pub async fn cancel_order(
        &self,
        request: &SignedApiRequest<CancelOrderSigned>,
    ) -> ApiClientResult<CancelResponse> {
        self.post_signed(request).await
    }
    pub async fn cancel_all_orders(
        &self,
        request: &SignedApiRequest<CancelAllOrdersSigned>,
    ) -> ApiClientResult<CancelAllResponse> {
        self.post_signed(request).await
    }
    /// Submit multiple individually signed place/cancel actions via `POST /order/batch`.
    ///
    /// The API verifies every action and rejects the whole request if any action
    /// is invalid; accepted actions are sequenced as normal single-action
    /// messages (not an on-chain atomic batch).
    pub async fn batch_orders(
        &self,
        request: &BatchOrderRequest,
    ) -> ApiClientResult<BatchOrderResponse> {
        self.post_json("/order/batch", request).await
    }

    pub async fn health(&self) -> ApiClientResult<serde_json::Value> {
        self.get_json("/health").await
    }
}

fn normalize_base_url(base_url: String) -> ApiClientResult<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() || !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(ApiClientError::InvalidBaseUrl(base_url));
    }
    Ok(trimmed.to_string())
}
