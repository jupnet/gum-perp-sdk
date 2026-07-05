use {
    crate::{
        CancelAllOrdersSigned, CancelOrderSigned, ClientMessage, Interval, PlaceOrderSigned,
        PostPayload, PostRequest, Pubkey, SignedApiRequest, Subscription, TradingActionSigned,
        WebsocketResponse,
    },
    futures::{SinkExt, StreamExt},
    thiserror::Error,
    tokio::net::TcpStream,
    tokio_tungstenite::{
        connect_async,
        tungstenite::{self, Bytes, Message},
        MaybeTlsStream, WebSocketStream,
    },
};

#[derive(Debug, Error)]
pub enum WebsocketClientError {
    #[error("invalid websocket/API URL: {0}")]
    InvalidUrl(String),
    #[error("websocket error: {0}")]
    Websocket(#[source] Box<tungstenite::Error>),
    #[error("failed to encode/decode websocket JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type WebsocketClientResult<T> = Result<T, WebsocketClientError>;

impl From<tungstenite::Error> for WebsocketClientError {
    fn from(err: tungstenite::Error) -> Self {
        Self::Websocket(Box::new(err))
    }
}

pub trait IntoTradingActionSigned {
    fn into_trading_action_signed(self) -> TradingActionSigned;
}

impl IntoTradingActionSigned for PlaceOrderSigned {
    fn into_trading_action_signed(self) -> TradingActionSigned {
        TradingActionSigned::Place(self)
    }
}
impl IntoTradingActionSigned for CancelOrderSigned {
    fn into_trading_action_signed(self) -> TradingActionSigned {
        TradingActionSigned::Cancel(self)
    }
}
impl IntoTradingActionSigned for CancelAllOrdersSigned {
    fn into_trading_action_signed(self) -> TradingActionSigned {
        TradingActionSigned::CancelAll(self)
    }
}
pub struct WebsocketClient {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WebsocketClient {
    pub fn websocket_url(base_url: &str) -> WebsocketClientResult<String> {
        let trimmed = base_url.trim().trim_end_matches('/');
        let url = if let Some(rest) = trimmed.strip_prefix("https://") {
            format!("wss://{}", append_ws_path(rest))
        } else if let Some(rest) = trimmed.strip_prefix("http://") {
            format!("ws://{}", append_ws_path(rest))
        } else if let Some(rest) = trimmed.strip_prefix("wss://") {
            format!("wss://{}", append_ws_path(rest))
        } else if let Some(rest) = trimmed.strip_prefix("ws://") {
            format!("ws://{}", append_ws_path(rest))
        } else {
            return Err(WebsocketClientError::InvalidUrl(base_url.to_string()));
        };
        Ok(url)
    }

    /// Connect to the production Gum Perps WebSocket ([`crate::GUM_API_URL`]).
    pub async fn connect() -> WebsocketClientResult<Self> {
        Self::connect_to(crate::GUM_API_URL).await
    }

    /// Connect to a custom API endpoint (staging, local development).
    pub async fn connect_to(base_url: &str) -> WebsocketClientResult<Self> {
        let url = Self::websocket_url(base_url)?;
        Self::connect_ws_url(&url).await
    }

    pub async fn connect_ws_url(ws_url: &str) -> WebsocketClientResult<Self> {
        let (stream, _) = connect_async(ws_url).await?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, message: &ClientMessage) -> WebsocketClientResult<()> {
        let text = serde_json::to_string(message)?;
        self.stream.send(Message::Text(text.into())).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self, subscription: Subscription) -> WebsocketClientResult<()> {
        self.send(&ClientMessage::Subscribe { subscription }).await
    }
    pub async fn unsubscribe(&mut self, subscription: Subscription) -> WebsocketClientResult<()> {
        self.send(&ClientMessage::Unsubscribe { subscription })
            .await
    }
    pub async fn subscribe_blockhash(&mut self) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::Blockhash).await
    }
    pub async fn subscribe_market_stats(&mut self) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::MarketStats).await
    }
    pub async fn subscribe_orderbook(
        &mut self,
        symbol: impl Into<String>,
        depth: Option<usize>,
        update_ms: Option<u64>,
    ) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::Orderbook {
            symbol: symbol.into(),
            depth,
            update_ms,
        })
        .await
    }
    pub async fn subscribe_trades(
        &mut self,
        symbol: impl Into<String>,
    ) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::Trades {
            symbol: symbol.into(),
        })
        .await
    }
    pub async fn subscribe_candles(
        &mut self,
        symbol: impl Into<String>,
        interval: Interval,
    ) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::Candle {
            symbol: symbol.into(),
            interval,
        })
        .await
    }
    pub async fn subscribe_positions(&mut self, pubkey: Pubkey) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::Positions { pubkey }).await
    }
    pub async fn subscribe_open_orders(&mut self, pubkey: Pubkey) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::OpenOrders { pubkey }).await
    }
    pub async fn subscribe_trading_account_events(
        &mut self,
        pubkey: Pubkey,
    ) -> WebsocketClientResult<()> {
        self.subscribe(Subscription::TradingAccountEvents { pubkey })
            .await
    }
    /// Send a WebSocket protocol-level ping frame; the server replies with a
    /// pong frame (consumed transparently by [`Self::next_response`]).
    pub async fn ping(&mut self, payload: impl Into<Bytes>) -> WebsocketClientResult<()> {
        self.stream.send(Message::Ping(payload.into())).await?;
        Ok(())
    }

    pub fn post_signed_message<T>(
        id: impl Into<String>,
        pubkey: Pubkey,
        request: SignedApiRequest<T>,
    ) -> ClientMessage
    where
        T: IntoTradingActionSigned,
    {
        ClientMessage::Post(Box::new(PostRequest {
            id: id.into(),
            payload: PostPayload {
                pubkey,
                action: request.body.into_trading_action_signed(),
            },
        }))
    }

    pub async fn post_signed<T>(
        &mut self,
        id: impl Into<String>,
        pubkey: Pubkey,
        request: SignedApiRequest<T>,
    ) -> WebsocketClientResult<()>
    where
        T: IntoTradingActionSigned,
    {
        self.send(&Self::post_signed_message(id, pubkey, request))
            .await
    }

    pub async fn next_response(&mut self) -> WebsocketClientResult<Option<WebsocketResponse>> {
        while let Some(message) = self.stream.next().await {
            match message? {
                Message::Text(text) => return Ok(Some(serde_json::from_str(&text)?)),
                Message::Binary(bytes) => return Ok(Some(serde_json::from_slice(&bytes)?)),
                Message::Close(_) => return Ok(None),
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            }
        }
        Ok(None)
    }

    pub async fn close(&mut self) -> WebsocketClientResult<()> {
        self.stream.close(None).await?;
        Ok(())
    }
}

fn append_ws_path(rest: &str) -> String {
    if rest.ends_with("/ws") {
        rest.to_string()
    } else {
        format!("{}/ws", rest.trim_end_matches('/'))
    }
}
