# WebSocket API Documentation

## Connection

Connect to the WebSocket endpoint:

```
wss://gum-api.jup.net/jtx/mainnet-beta/ws
```

## Message Format

Client messages use JSON format with a `method` field as the discriminator.
Server messages are enveloped with `type` and `data`:

```json
{
  "type": "blockhash",
  "data": {
    "blockhash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
  }
}
```

The subscription data examples below show the `data` object unless the full envelope is shown.

### Methods

| Method | Description |
|--------|-------------|
| `subscribe` | Subscribe to a data stream |
| `unsubscribe` | Unsubscribe from a data stream |
| `get` | Read-only queries (e.g., ping) |
| `post` | Write operations (trading actions) |

---

## Subscribe / Unsubscribe

### Blockhash Subscription

Subscribe to receive the latest blockhash updates (sent every ~200ms).

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "blockhash"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "blockhash"
  }
}
```

**Subscription Data:**
```json
{
  "blockhash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

---

### Trading Account Events Subscription

Subscribe to receive events for a specific trading account (deposits, withdrawals, order fills, cancellations, etc.).

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "tradingAccountEvents",
    "pubkey": "ABC123..."
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "tradingAccountEvents",
    "pubkey": "ABC123..."
  }
}
```

**Subscription Data:**

Events are sent as `JTXApiEvent` objects with the following types:

| Event Type | Description |
|------------|-------------|
| `Deposit` | Funds deposited to trading account |
| `Withdraw` | Funds withdrawn from trading account |
| `Submit` | Order placed on the orderbook |
| `Fill` | Order filled (partial or complete) |
| `Cancel` | Order cancelled |
| `Settle` | PnL settled |
| `SettleFunding` | Funding settled |
| `SettleFundingCollateral` | Funding collateral settled |
| `SyncCollateral` | Collateral moved between the user's cross-margin and isolated-margin accounts. Sent to subscribers of both the source and destination accounts. |
| `Error` | Order rejected or failed on-chain |

#### Submit (Place) Event

Submit events are sent when an order is placed on the orderbook.

```json
{
  "action": "Submit",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "market": "SOL",
  "marketPubkey": "ABC123...",
  "maker": "DEF456...",
  "baseLots": 100,
  "pricePerLot": 15000,
  "orderId": "123456789",
  "clientOrderId": 12345,
  "side": "Ask",
  "timestamp": 1702656000,
  "expiryTimestamp": 0,
  "orderType": "limit",
  "timeInForce": "gtc",
  "reduceOnly": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `market` | string | Market symbol (e.g., `"SOL"`). Falls back to the market public key if the symbol cannot be resolved. |
| `marketPubkey` | string | Market public key |
| `maker` | string | Maker's trading account public key |
| `baseLots` | number | Order quantity in base lots |
| `pricePerLot` | number | Order price in price lots |
| `orderId` | string | Order ID (u128 as string) |
| `clientOrderId` | number | Client-provided order ID (0 if not set) |
| `side` | string | Order side: `"Bid"` or `"Ask"` |
| `timestamp` | number | Unix timestamp (seconds) |
| `expiryTimestamp` | number | Expiry timestamp (0 = no expiry) |
| `orderType` | string | Order type: `"limit"`, `"market"`, or `"postOnly"` |
| `timeInForce` | string | Time in force: `"gtc"`, `"gtd"`, `"ioc"`, `"fok"`, `"liquidation"`, or `"adl"` |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |

#### Fill Event

Fill events are sent when an order is partially or fully filled.

```json
{
  "action": "Fill",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "market": "SOL",
  "marketPubkey": "ABC123...",
  "makerId": 42,
  "taker": "DEF456...",
  "baseLots": 100,
  "pricePerLot": 15000,
  "makerOrderId": "123456789",
  "takerSeqNum": 456,
  "side": "Ask",
  "timestamp": 1702656000,
  "takerEntryPricePerLot": 14500,
  "clientOrderId": 12345
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `market` | string | Market symbol (e.g., `"SOL"`). Falls back to the market public key if the symbol cannot be resolved. |
| `marketPubkey` | string | Market public key |
| `makerId` | number | Maker's trading account ID |
| `taker` | string | Taker's trading account public key |
| `baseLots` | number | Fill quantity in base lots |
| `pricePerLot` | number | Fill price in price lots |
| `makerOrderId` | string | Maker's order ID (u128 as string) |
| `takerSeqNum` | number | Taker's order sequence number |
| `side` | string | Maker side: `"Bid"` or `"Ask"`. When sent to the taker, this is flipped to the taker's perspective |
| `timestamp` | number | Unix timestamp (seconds) |
| `takerEntryPricePerLot` | number | Taker's average entry price in price lots at the time of the fill. `0` if the fill opened or increased a position |
| `clientOrderId` | number \| null | Client-provided order ID (maker's when sent to maker, taker's when sent to taker) |

#### Cancel Event

Cancel events are sent whenever a resting order is removed from the orderbook.

```json
{
  "action": "Cancel",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "market": "SOL",
  "marketPubkey": "ABC123...",
  "makerId": 42,
  "baseLots": 100,
  "pricePerLot": 15000,
  "orderId": "123456789",
  "side": "Ask",
  "timestamp": 1702656000,
  "reason": "UserRequest",
  "clientOrderId": 12345
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `market` | string | Market symbol (e.g., `"SOL"`). Falls back to the market public key if the symbol cannot be resolved. |
| `marketPubkey` | string | Market public key |
| `makerId` | number | Maker's trading account ID |
| `baseLots` | number | Cancelled (remaining) quantity in base lots |
| `pricePerLot` | number | Order price in price lots |
| `orderId` | string | Order ID (u128 as string) |
| `side` | string | Order side: `"Bid"` or `"Ask"` |
| `timestamp` | number | Unix timestamp (seconds) |
| `reason` | string | Why the order was cancelled (see below) |
| `clientOrderId` | number \| null | Client-provided order ID, if one was set on the cancelled order |

**`reason` values:**

| Value | When |
|-------|------|
| `"UserRequest"` | Explicit cancel by the order owner |
| `"CancelAll"` | Owner cancelled all orders for the market |
| `"SelfTradePrevention"` | Owner placed a new order that would have matched their own resting order |
| `"Expired"` | Order's `expiryTimestamp` elapsed |
| `"ImmediateOrCancel"` | Unfilled remainder of an IOC (or liquidation) order |
| `"MatchLimitReached"` | Matching halted before fill due to compute/heap limits, so the remainder was not posted |
| `"Eviction"` | Orderbook full — the worst-priced order was evicted to make room for an incoming better-priced one |
| `"ReduceOnlyInvalid"` | Existing reduce-only order was invalidated by a same-side order placement |
| `"Liquidation"` | Cancelled during a liquidation cascade for the owner |
| `"Invalid"` | SDK fallback for an unknown numeric cancellation reason |

#### SyncCollateral Event

Emitted when the `sync_collateral` instruction moves USDC between a user's cross-margin and isolated-margin trading accounts. Both source and destination subscribers receive a copy.

```json
{
  "action": "SyncCollateral",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "source": "ABC123...",
  "destination": "DEF456...",
  "amount": 1000000,
  "direction": "CrossToIsolated",
  "timestamp": 1702656000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `source` | string | Trading account public key collateral was moved from |
| `destination` | string | Trading account public key collateral was moved to |
| `amount` | number | USDC amount transferred in native quote units |
| `direction` | string | `"CrossToIsolated"` or `"IsolatedToCross"` |
| `timestamp` | number | Unix timestamp (seconds) |

#### Error Event

Error events are sent when an order is rejected or fails on-chain (e.g., insufficient margin, post-only order would match).

```json
{
  "type": "Error",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "error": "PostOnlyOrderWouldMatch",
  "clientOrderId": 12345
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `error` | string | Error message (e.g., `PostOnlyOrderWouldMatch`, `OrderNotFound`, `InsufficientMargin`) |
| `clientOrderId` | number \| null | Client-provided order ID if one was included in the original request |

---

### Positions Subscription

Subscribe to receive real-time position updates for a specific trading account. A full snapshot is broadcast every ~2 seconds while positions exist, and per-position updates fire on each fill the account participates in. When a fill closes a position, a zero-size update (`size: "0"`, `avgEntryPrice: "0"`, etc.) is emitted on the closed market so clients can clear local state — REST `GET /tradingAccount/{trading_account}/positions` continues to omit closed positions.

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "positions",
    "pubkey": "TRADING_ACCOUNT_PUBKEY"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "positions",
    "pubkey": "TRADING_ACCOUNT_PUBKEY"
  }
}
```

**Subscription Data:**
```json
{
  "symbol": "SOL",
  "side": "Bid",
  "size": "10.5",
  "avgEntryPrice": "100.50",
  "markPrice": "102.625",
  "unrealizedPnl": "25.00",
  "unrealizedPnlPercent": "2.49",
  "margin": "10.50",
  "liquidationPrice": "90.00",
  "fundingAccrued": "1.25",
  "pendingFundingPayable": "0",
  "pendingFundingReceivable": "1.25"
}
```

**Data Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `side` | string | Position side: `"Bid"` for long or `"Ask"` for short |
| `size` | string | Position size in base units |
| `avgEntryPrice` | string | Average entry price |
| `markPrice` | string | Mark price used for unrealized PnL, margin, and liquidation estimates |
| `unrealizedPnl` | string | Position-only unrealized PnL based on entry cost and current mark price (excludes realized PnL from partial closes) |
| `unrealizedPnlPercent` | string | Unrealized PnL as percentage of entry value |
| `margin` | string | Initial margin requirement for this position |
| `liquidationPrice` | string | Estimated liquidation price (cross-margin) |
| `fundingAccrued` | string | Unsettled funding accrued on this position (USDC). Positive = funding received, negative = funding paid |
| `pendingFundingPayable` | string | Pending funding this position still owes (USDC, >= 0). Mirrors the negative side of `fundingAccrued` |
| `pendingFundingReceivable` | string | Pending funding this position is still owed (USDC, >= 0). A pending receivable only — not claimable, transferable, or withdrawable until actually settled |

---

### Open Orders Subscription

Subscribe to receive real-time updates for a trading account's open orders. Each order update is sent individually as changes occur.

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "openOrders",
    "pubkey": "TRADING_ACCOUNT_PUBKEY"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "openOrders",
    "pubkey": "TRADING_ACCOUNT_PUBKEY"
  }
}
```

**Subscription Data:**
```json
{
  "symbol": "SOL",
  "orderId": "123456789",
  "side": "Bid",
  "price": "100.50",
  "initialSize": "10.0",
  "remainingSize": "5.0",
  "expiryTimestamp": 1702656000,
  "status": "partially_filled",
  "clientOrderId": 12345,
  "reduceOnly": false
}
```

**Data Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `orderId` | string | Order ID (u128 as string) |
| `side` | string | Order side: `"Bid"` or `"Ask"` |
| `price` | string | Order price |
| `initialSize` | string | Original order size when placed |
| `remainingSize` | string | Unfilled portion of the order |
| `expiryTimestamp` | number | Unix timestamp when order expires (0 = no expiry) |
| `status` | string | Order status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"` |
| `clientOrderId` | number | Optional client-provided order ID (omitted if not set) |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |

---

### Trades Subscription

Subscribe to receive all trades for a specific market symbol.

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "trades",
    "symbol": "BTC"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "trades",
    "symbol": "BTC"
  }
}
```

**Subscription Data:**
```json
{
  "symbol": "BTC",
  "side": "Bid",
  "price": "50000.00",
  "size": "1.5",
  "txid": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "time": 1702656000000,
  "tradingAccounts": ["buyer_pubkey", "seller_pubkey"]
}
```

---

### Orderbook Subscription

Subscribe to receive orderbook updates for a specific market symbol.

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "orderbook",
    "symbol": "BTC",
    "depth": 25,
    "updateMs": 100
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "orderbook",
    "symbol": "BTC",
    "depth": 25,
    "updateMs": 100
  }
}
```

**Subscription Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., `"BTC"`) |
| `depth` | integer | Optional levels per side. Defaults to `10`. Rounded up to the nearest tier: `5`, `10`, `25`, `50`, `100`. Must match on unsubscribe. |
| `updateMs` | integer | Optional update interval in milliseconds. Defaults to `500`. Rounded up to the nearest tier: `0` (real-time, every change), `100`, `500`, `1000`. |

**Subscription Data:**
```json
{
  "symbol": "BTC",
  "bids": [[50000.0, 1.5], [49999.0, 2.0]],
  "asks": [[50001.0, 1.0], [50002.0, 3.0]]
}
```

- `bids`: Array of `[price, size]` sorted by price descending (best bid first)
- `asks`: Array of `[price, size]` sorted by price ascending (best ask first)

---

### Candle Subscription

Subscribe to receive real-time candlestick updates for a specific market and interval (sent every ~1 second when data changes).

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "candle",
    "symbol": "BTC",
    "interval": "1m"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "candle",
    "symbol": "BTC",
    "interval": "1m"
  }
}
```

**Subscription Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "BTC") |
| `interval` | string | Candle interval: `"1m"`, `"5m"`, `"15m"`, `"1h"`, `"4h"`, `"1d"` |

**Subscription Data:**
```json
{
  "t": 1702656000000,
  "T": 1702656060000,
  "s": "BTC",
  "i": "1m",
  "o": 50000.0,
  "c": 50100.0,
  "h": 50150.0,
  "l": 49950.0,
  "v": 100.5,
  "n": 25
}
```

**Data Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `t` | number | Open time (milliseconds) |
| `T` | number | Close time (milliseconds) |
| `s` | string | Symbol |
| `i` | string | Interval |
| `o` | number | Open price |
| `c` | number | Close price |
| `h` | number | High price |
| `l` | number | Low price |
| `v` | number | USD notional volume (quantity × price) |
| `n` | number | Number of trades |

---

### Market Stats Subscription

Subscribe to a periodic snapshot of index/mark price, open interest, funding rate,
and 24h price change for **all markets**. One push every 5 seconds.

**Request:**
```json
{
  "method": "subscribe",
  "subscription": {
    "type": "marketStats"
  }
}
```

**Unsubscribe:**
```json
{
  "method": "unsubscribe",
  "subscription": {
    "type": "marketStats"
  }
}
```

**Subscription Data:**
```json
{
  "type": "marketStats",
  "data": {
    "timestamp": 1745793600,
    "markets": [
      {
        "symbol": "SOL",
        "indexPrice": 102.625,
        "markPrice": 102.625,
        "openInterest": 125050.0,
        "openInterestBase": 1250.5,
        "fundingRateBps1h": 0.42,
        "priceChange24h": 2.5,
        "priceChangePercent24h": 2.5
      }
    ]
  }
}
```

**Data Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | number | Push time (unix seconds) |
| `markets[].symbol` | string | Market symbol |
| `markets[].indexPrice` | number | Current index/oracle valuation price |
| `markets[].markPrice` | number | Current risk price used for margin, liquidation estimates, unrealized PnL, and withdrawal health. In phase one this equals `indexPrice`. |
| `markets[].openInterest` | number | Open interest notional in quote currency |
| `markets[].openInterestBase` | number | Open interest in base units |
| `markets[].fundingRateBps1h` | number \| null | Estimated 1h funding rate (bps); `null` when the cache has no fresh estimate for the current hour |
| `markets[].priceChange24h` | number | Absolute 24h price change in quote currency |
| `markets[].priceChangePercent24h` | number | 24h price change percent |

---

### Subscription Response

All subscribe/unsubscribe requests receive a confirmation response:

**Success:**
```json
{
  "type": "subscription",
  "data": {
    "method": "subscribe",
    "subscription": {
      "type": "blockhash"
    },
    "success": true
  }
}
```

**Error:**
```json
{
  "type": "subscription",
  "data": {
    "method": "subscribe",
    "subscription": {
      "type": "trades",
      "symbol": "INVALID-SYMBOL"
    },
    "success": false,
    "error": "Unknown market symbol: INVALID-SYMBOL"
  }
}
```

---

## Get (Read-Only Queries)

### Ping

Check if the connection is alive.

**Request:**
```json
{
  "method": "get",
  "id": "my-request-id",
  "payload": {
    "type": "ping"
  }
}
```

**Response:**
```json
{
  "type": "request",
  "data": {
    "id": "my-request-id",
    "success": true,
    "data": {
      "pong": true
    }
  }
}
```

---

## Post (Trading Actions)

Trading actions require a signed payload. The `id` field is returned in the response for request correlation.

### Place Order

**Request:**
```json
{
  "method": "post",
  "id": "order-123",
  "payload": {
    "pubkey": "USER_PUBKEY",
    "action": {
      "method": "place",
      "symbol": "SOL",
      "side": "Bid",
      "orderType": "limit",
      "price": 100.0,
      "quantity": 1.5,
      "reduceOnly": false,
      "timeInForce": "GTC",
      "clientOrderId": 12345,
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    }
  }
}
```

**Action Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `method` | string | Yes | `"place"` |
| `subaccountId` | number | No | Subaccount index (0-255). Defaults to `0` |
| `symbol` | string | Yes | Market symbol (e.g., "SOL") |
| `side` | string | Yes | `"Bid"`/`"Ask"`; lowercase aliases `"bid"`/`"ask"` and `"buy"`/`"sell"` are also accepted |
| `orderType` | string | No | `"limit"` (default), `"postOnly"`, or `"market"` |
| `price` | number | Conditional | `> 0` for `limit` and `postOnly`. For `market`: omitted/`0` sweeps at any price; `> 0` acts as a slippage cap |
| `quantity` | number | Yes | Quantity in base units (e.g., 1.5) |
| `reduceOnly` | boolean | No | If true, only reduce existing position |
| `timeInForce` | string/object | No | Defaults to `"ImmediateOrCancel"` for `market` and `"GoodTilCanceled"` for `limit`/`postOnly`. Market orders accept `IOC` or `FOK`; post-only orders reject `IOC` and `FOK` |
| `clientOrderId` | number | No | Optional client-provided order ID |
| `collateralAmount` | number | No | Routes the order to an **isolated-margin** account. When present, this many USDC native units are moved from the cross-margin parent into the isolated subaccount alongside the order; `0` moves no fresh collateral (the order draws on the isolated account's existing position/reservation). Omit it entirely for a plain cross-margin order. The `tradingAccount` must be the matching isolated PDA |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

> **Isolated vs cross margin.** Omitting `collateralAmount` places a cross-margin order (the `place_order` instruction). Including `collateralAmount` (even `0`) places an isolated-margin order (the `place_order_isolated` instruction): the filled portion is backed by the isolated account's own collateral, while any resting remainder is fenced against the collateral reserved for the order — it is *not* backed by the rest of the isolated account's balance. An isolated order whose reservation is too small for its notional is rejected on-chain with `InsufficientMargin`.
>
> Market orders use `price = 0` to sweep at any price; `> 0` acts as a slippage cap. Unfilled remainder is cancelled (IOC). Order history surfaces these as `orderType: "market"` with `price = "0"` (sweep) or the formatted cap.
> Post-only orders use `orderType: "postOnly"` and reject if they would match immediately.

**TimeInForce Values:**

| Value | Aliases | Description |
|-------|---------|-------------|
| `"GoodTilCanceled"` | `"GTC"` | Order remains active until canceled (default) |
| `"ImmediateOrCancel"` | `"IOC"` | Fill immediately, cancel unfilled portion |
| `"FillOrKill"` | `"FOK"` | Fill entirely or cancel completely |
| `{"Seconds": N}` | - | Order expires after N seconds (N must be > 0) |

---

### Cancel Order

Cancel a specific order by order ID or client order ID.

**Request:**
```json
{
  "method": "post",
  "id": "cancel-123",
  "payload": {
    "pubkey": "USER_PUBKEY",
    "action": {
      "method": "cancel",
      "symbol": "SOL",
      "orderId": "123456789",
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    }
  }
}
```

**Action Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `method` | string | Yes | `"cancel"` |
| `subaccountId` | number | No | Subaccount index. Defaults to `0` |
| `isIsolated` | boolean | No | Target an isolated-margin account instead of cross-margin. Defaults to `false`. Must match the `tradingAccount`'s margin type or the request is rejected on-chain |
| `symbol` | string | Yes | Market symbol |
| `orderId` | string | No | Order ID to cancel (u128 as string) |
| `clientOrderId` | number | No | Client order ID to cancel |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash |
| `signature` | string | Yes | User's signature |

*Note: Provide either `orderId` or `clientOrderId`, not both.*

---

### Cancel All Orders

Cancel all open orders for a market.

**Request:**
```json
{
  "method": "post",
  "id": "cancel-all-123",
  "payload": {
    "pubkey": "USER_PUBKEY",
    "action": {
      "method": "cancelAll",
      "symbol": "SOL",
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    }
  }
}
```

**Action Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `method` | string | Yes | `"cancelAll"` |
| `subaccountId` | number | No | Subaccount index. Defaults to `0` |
| `isIsolated` | boolean | No | Target an isolated-margin account instead of cross-margin. Defaults to `false`. Must match the `tradingAccount`'s margin type or the request is rejected on-chain |
| `symbol` | string | Yes | Market symbol |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash |
| `signature` | string | Yes | User's signature |

---

### Request Response

All get/post requests receive a response:

**Success:**
```json
{
  "type": "request",
  "data": {
    "id": "order-123",
    "success": true,
    "data": {
      "symbol": "SOL",
      "clientOrderId": 12345
    }
  }
}
```

**Error:**
```json
{
  "type": "request",
  "data": {
    "id": "order-123",
    "success": false,
    "error": "Insufficient margin"
  }
}
```

---

## Summary

| Method | Subscription/Payload Type | Description |
|--------|---------------------------|-------------|
| `subscribe` | `blockhash` | Latest blockhash updates |
| `subscribe` | `tradingAccountEvents` | Trading account specific events |
| `subscribe` | `positions` | Real-time position updates |
| `subscribe` | `openOrders` | Real-time open order updates |
| `subscribe` | `trades` | Market trades |
| `subscribe` | `orderbook` | Orderbook snapshots |
| `subscribe` | `candle` | Candlestick/OHLCV updates |
| `subscribe` | `marketStats` | Oracle / OI / funding / 24h change for all markets (5s) |
| `unsubscribe` | (same as subscribe) | Stop receiving updates |
| `get` | `ping` | Connection health check |
| `post` | `place` | Place a new order |
| `post` | `cancel` | Cancel an order |
| `post` | `cancelAll` | Cancel all orders |
