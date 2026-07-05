# REST API Documentation

## Base URL

```
https://gum-api.jup.net/jtx/mainnet-beta
```

## Authentication

Most endpoints require the `X-PUBKEY` header containing the user's public key.

```
X-PUBKEY: YOUR_PUBLIC_KEY
```

### Signing Trading Actions

Trading write endpoints (`/order/create`, `/order/cancel`, `/order/cancel-all`, and `/order/batch`) require signed action payloads. The request body includes the action fields plus:

| Field | Description |
|-------|-------------|
| `tradingAccount` | Trading account public key the action applies to |
| `recentBlockhash` | Recent blockhash used when signing |
| `signature` | User signature over the serialized JTX action |

The `X-PUBKEY` header must be the public key for the signing keypair. The API reconstructs the same action from the JSON body, serializes it with the Jupnet action format, and verifies `signature` against `X-PUBKEY`.

Recommended signing flow:

1. Fetch market metadata with `GET /market/{symbol}`. The SDK uses `baseDecimals`, `baseLotSize`, and `quoteLotSize` to convert decimal prices and quantities to on-chain lots.
2. Fetch a blockhash with `GET /blockhash` or subscribe to the WebSocket `blockhash` stream.
3. Build the unsigned SDK action (`PlaceOrder`, `CancelOrder`, or `CancelAllOrders`).
4. Sign the action with the same market metadata and blockhash (`sign_request_with(...)` in the Rust SDK). For `/order/batch`, sign each `place` / `cancel` action individually and include those signed action bodies in the batch vector.
5. Submit the signed JSON body with `X-PUBKEY` set to the signer public key.

Rust SDK example (using the [`gum-perp-sdk`](../rust) crate):

```rust
use std::num::NonZeroU64;

use gum_perp_sdk::{
    Keypair, MarketSymbol, OrderType, PlaceOrder, Pubkey, RestClient, Side,
    SignableTradingRequest, TimeInForce,
};

let signer = Keypair::from_bytes(&std::fs::read("user-keypair.bin")?)?;
let user_pubkey = signer.pubkey();
let trading_account: Pubkey = "TRADING_ACCOUNT_PUBKEY".parse()?;
let client = RestClient::new(user_pubkey);

let market = client.market("SOL").await?;
let blockhash = client.blockhash().await?.blockhash;

let order = PlaceOrder {
    subaccount_id: 0,
    symbol: MarketSymbol("SOL".to_string()),
    side: Side::Bid,
    order_type: OrderType::Limit,
    price: 100.50,
    quantity: 10.0,
    reduce_only: false,
    time_in_force: TimeInForce::GoodTilCanceled,
    client_order_id: Some(NonZeroU64::new(12345).unwrap()),
    collateral_amount: None,
};

let signed = order.sign_request_with(trading_account, &market, blockhash, &signer)?;
let response = client.place_order(&signed).await?;
```

For external HSM/KMS signers, build the signable bytes with
`order.message_to_sign(user_pubkey, &market, blockhash)?`, sign them externally, and
assemble the request with `order.signed_request(trading_account, blockhash, signature)`.

If you are not using the Rust SDK, do not sign the JSON request body. Sign the binary Jupnet `Action` serialization with:

| Action field | Value |
|--------------|-------|
| `programId` | JTX program id |
| `signers` | Single-element array containing `X-PUBKEY` |
| `recentBlockhash` | Same value submitted in the request body |
| `data` | One-byte JTX instruction discriminator followed by Borsh-serialized instruction params |

The instruction params must match the API's conversion from decimal request fields to lots. Using the SDK avoids mismatches in lot conversion, action serialization, and typed signature formatting.

The API validates `recentBlockhash` against an in-memory cache of recently observed blockhashes before publishing the order. Requests that present a blockhash the API has not seen, or one whose `last_valid_blockheight` has already passed, are rejected with HTTP `400` (`"Stale or unknown recent blockhash"`). Fetch the blockhash immediately before signing (`GET /blockhash` or the WebSocket `blockhash` stream) and avoid caching it across long windows.

### Signing Deposit, Withdraw, and Transfer Transactions

Deposit, withdraw, and transfer use a different signing flow from orders:

1. Call `GET /deposit`, `GET /withdraw`, or `GET /transfer` with `X-PUBKEY` and the required query parameters.
2. Decode the returned `serializedTransaction` from base64.
3. Sign the transaction message bytes at the required signature slot where the account key equals `X-PUBKEY`.
4. Re-serialize the transaction, base64-encode it, and submit it to the matching `POST` endpoint.

The server validates that the transaction contains the expected JTX instruction and that the user account matches `X-PUBKEY`, then adds the server fee-payer signature before submitting it.

---

## Markets

### List Markets

Get all available markets.

**Endpoint:** `GET /markets`

**Response:**
```json
[
  {
    "symbol": "SOL",
    "pubkey": "ABC123...",
    "baseDecimals": 9,
    "baseLotSize": 10000000,
    "quoteLotSize": 100,
    "tickSize": 0.01,
    "stepSize": 0.01,
    "indexPrice": 102.625,
    "markPrice": 102.625,
    "bestBid": 102.50,
    "bestAsk": 102.75,
    "midPrice": 102.625,
    "initialMarginRatioBps": 1000,
    "maintenanceMarginRatioBps": 500,
    "timestamp": 1702656000,
    "lastUpdatedMs": 100,
    "openInterest": 125050.0,
    "openInterestBase": 1250.5,
    "high24h": 105.25,
    "low24h": 98.50,
    "volume24h": 50000.0,
    "lastTradePrice": 102.75,
    "priceChange24h": 2.50,
    "priceChangePercent24h": 2.49,
    "statsLastUpdatedMs": 1702656000000
  }
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |
| `pubkey` | string | Market account public key |
| `baseDecimals` | number | Decimal places for base asset |
| `baseLotSize` | number | Minimum base quantity in native units |
| `quoteLotSize` | number | Minimum quote quantity in native units |
| `tickSize` | number | Minimum price increment (e.g., 0.01 = $0.01) |
| `stepSize` | number | Minimum quantity increment (e.g., 0.01 = 0.01 SOL) |
| `indexPrice` | number | Current index/oracle valuation price |
| `markPrice` | number | Current risk price used for margin, liquidation estimates, unrealized PnL, and withdrawal health. In phase one this equals `indexPrice`. |
| `bestBid` | number | Best bid price (0 if no bids) |
| `bestAsk` | number | Best ask price (0 if no asks) |
| `midPrice` | number | Mid price (average of best bid and ask, 0 if either side is empty) |
| `initialMarginRatioBps` | number | Initial margin requirement in basis points (e.g., 1000 = 10%) |
| `maintenanceMarginRatioBps` | number | Maintenance margin requirement in basis points (e.g., 500 = 5%) |
| `timestamp` | number | Unix timestamp of last update |
| `lastUpdatedMs` | number | Milliseconds since last update |
| `openInterest` | number | Total open interest notional value in quote currency (base quantity × price) |
| `openInterestBase` | number | Total open interest in base units (sum of all long positions) |
| `high24h` | number | 24-hour high price |
| `low24h` | number | 24-hour low price |
| `volume24h` | number | 24-hour trading volume in base units |
| `lastTradePrice` | number | Last trade price (0 if no trades) |
| `priceChange24h` | number | Price change in last 24 hours |
| `priceChangePercent24h` | number | Price change percentage in last 24 hours |
| `statsLastUpdatedMs` | number | Timestamp when stats were last computed (Unix ms) |

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/markets
```

---

### Get Market

Get information for a specific market.

**Endpoint:** `GET /market/{symbol}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Response:**
```json
{
  "symbol": "SOL",
  "pubkey": "ABC123...",
  "baseDecimals": 9,
  "baseLotSize": 10000000,
  "quoteLotSize": 100,
  "tickSize": 0.01,
  "stepSize": 0.01,
  "indexPrice": 102.625,
  "markPrice": 102.625,
  "bestBid": 102.50,
  "bestAsk": 102.75,
  "midPrice": 102.625,
  "initialMarginRatioBps": 1000,
  "maintenanceMarginRatioBps": 500,
  "timestamp": 1702656000,
  "lastUpdatedMs": 100,
  "openInterest": 125050.0,
  "openInterestBase": 1250.5,
  "high24h": 105.25,
  "low24h": 98.50,
  "volume24h": 50000.0,
  "lastTradePrice": 102.75,
  "priceChange24h": 2.50,
  "priceChangePercent24h": 2.49,
  "statsLastUpdatedMs": 1702656000000
}
```

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/market/SOL
```

---

### Get Orderbook

Get the current orderbook for a market.

**Endpoint:** `GET /orderbook/{symbol}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `depth` | number | 10 | Number of price levels per side (1-500) |

**Response:**
```json
{
  "symbol": "SOL",
  "bids": [[100.50, 10.5], [100.00, 20.0]],
  "asks": [[101.00, 5.0], [101.50, 15.0]],
  "timestamp": 1702656000,
  "lastUpdatedMs": 100
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `bids` | array | Array of `[price, size]` sorted by price descending |
| `asks` | array | Array of `[price, size]` sorted by price ascending |
| `timestamp` | number | Unix timestamp |
| `lastUpdatedMs` | number | Milliseconds since last update |

**Examples:**
```bash
# Default (10 levels per side)
curl https://gum-api.jup.net/jtx/mainnet-beta/orderbook/SOL

# Full orderbook (up to 500 levels per side)
curl https://gum-api.jup.net/jtx/mainnet-beta/orderbook/SOL?depth=500
```

---

## Trades

### Get Recent Trades

Get the last 100 trades for a market.

**Endpoint:** `GET /trades/{symbol}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Response:**
```json
{
  "symbol": "SOL",
  "trades": [
    {
      "symbol": "SOL",
      "side": "Bid",
      "price": "100.50",
      "size": "10.5",
      "txid": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
      "time": 1702656000,
      "tradingAccounts": ["buyer_pubkey", "seller_pubkey"]
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `side` | string | Taker side: `"Bid"` (buy) or `"Ask"` (sell) |
| `price` | string | Trade price |
| `size` | string | Trade size in base units |
| `txid` | string | Transaction signature |
| `time` | number | Unix timestamp |
| `tradingAccounts` | array | `[buyer_pubkey, seller_pubkey]` |

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/trades/SOL
```

---

## OHLCV (Candlestick Data)

### Get OHLCV

Get historical candlestick data for charting.

**Endpoint:** `GET /ohlcv/{symbol}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `interval` | string | Yes | Candle interval: `"1m"`, `"5m"`, `"15m"`, `"1h"`, `"4h"`, `"1d"` |
| `from` | number | Yes | Start timestamp (Unix seconds) |
| `to` | number | Yes | End timestamp (Unix seconds) |

**Response:**
```json
[
  {
    "t": 1702656000000,
    "T": 1702659600000,
    "s": "SOL",
    "i": "1h",
    "o": 100.0,
    "c": 101.5,
    "h": 102.0,
    "l": 99.5,
    "v": 1000.5,
    "n": 150
  }
]
```

**Response Fields:**

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

**Limits:**
- Maximum 1000 candles per request
- `from` must be less than `to`

**Example:**
```bash
curl "https://gum-api.jup.net/jtx/mainnet-beta/ohlcv/SOL?interval=1h&from=1702656000&to=1702742400"
```

---

## Funding Rates

### List Funding Rates

Get current funding rates for all markets.

**Endpoint:** `GET /fundingRates`

**Response:**
```json
{
  "fundingRates": [
    {
      "symbol": "SOL",
      "rateBps1h": 0.654,
      "fundingRate": "0.0075",
      "predictedFundingRate": "0.0075",
      "fundingIntervalSeconds": 28800,
      "nextFundingCollectionTime": 1709164800,
      "nextFundingDistributionTime": 1709139600,
      "fundingUpdateTimestamp": 1709136060
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `fundingRates` | array | Array of funding rate entries |
| `fundingRates[].symbol` | string | Market symbol |
| `fundingRates[].rateBps1h` | number | 1-hour funding rate in basis points, derived from the latest on-chain rate |
| `fundingRates[].fundingRate` | string | Latest final funding rate for the current interval as a decimal (e.g. `"0.0075"` = 0.75% per interval) |
| `fundingRates[].predictedFundingRate` | string | Current projected rate from the same formula. The chain recomputes the rate on every update, so this equals `fundingRate` |
| `fundingRates[].fundingIntervalSeconds` | number | User-facing funding period: the rate's period basis — not a payment schedule. The premium-averaging window follows the hourly distribution interval, not this interval |
| `fundingRates[].nextFundingCollectionTime` | number | Unix timestamp of the next funding period boundary (UTC-aligned to the interval). Settlement health is evaluated against it; funding is not paid out in a lump at this time |
| `fundingRates[].nextFundingDistributionTime` | number | Unix timestamp of the next distributed-index update (UTC hour-aligned) — when accrued funding becomes position-visible and lands on accounts |
| `fundingRates[].fundingUpdateTimestamp` | number | Unix timestamp of the last successful on-chain funding update |

**Notes:**
- Funding follows the P0-6 formula: impact bid/ask premium index → time-weighted average premium → interest-clamped adjustment → interval scaling → per-market cap/floor
- Positive rate means longs pay shorts; negative means shorts pay longs
- Funding accrues continuously and is applied to accounts on the distribution cadence (hourly), not in a lump at the period boundary. A "next funding payment" countdown should use `nextFundingDistributionTime`; `nextFundingCollectionTime` marks the period boundary for rate averaging and settlement-health monitoring
- `rateBps1h` returns 0 if no cached estimate is available for the current hour

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/fundingRates
```

---

### Get Funding Rate

Get the current funding rate for a specific market.

**Endpoint:** `GET /fundingRate/{symbol}`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Response:**
```json
{
  "symbol": "SOL",
  "rateBps1h": 0.654,
  "fundingRate": "0.0075",
  "predictedFundingRate": "0.0075",
  "fundingIntervalSeconds": 28800,
  "nextFundingCollectionTime": 1709164800,
  "nextFundingDistributionTime": 1709139600,
  "fundingUpdateTimestamp": 1709136060
}
```

**Response Fields:** Same as `fundingRates[]` entry above.

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/fundingRate/SOL
```

---

### Get Funding Rate History

Get historical hourly funding rate deltas for a market.

**Endpoint:** `GET /fundingRate/{symbol}/history`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `symbol` | string | Market symbol (e.g., "SOL") |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | number | No | Maximum entries to return (default: 100, max: 500) |
| `offset` | number | No | Offset for pagination (default: 0) |
| `startTime` | number | No | Unix timestamp - filter entries at or after this time |
| `endTime` | number | No | Unix timestamp - filter entries at or before this time |

**Response:**
```json
{
  "entries": [
    {
      "symbol": "SOL",
      "rateBps1h": 1.25,
      "timestamp": 1709136000
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `entries` | array | Array of funding rate history entries |
| `entries[].symbol` | string | Market symbol |
| `entries[].rateBps1h` | number | Hourly funding rate in basis points |
| `entries[].timestamp` | number | Unix timestamp (seconds) of the UTC hour-aligned funding distribution period |

**Notes:**
- One entry per UTC hour in which a funding distribution fired on-chain
- `timestamp` is the canonical funding distribution period (`HH:00:00`), not the transaction block time; the transaction may land a few seconds later due to slot/RPC timing
- `rateBps1h` is derived from the exact on-chain accrual: `(long_distributed_index(t) − long_distributed_index(t−1h)) * 10_000 / base_lot_value` (the first historical entry, with no prior distribution row, is excluded)
- Results are sorted by time descending

**Example:**
```bash
# Get latest funding rate history
curl https://gum-api.jup.net/jtx/mainnet-beta/fundingRate/SOL/history

# Get history with pagination
curl "https://gum-api.jup.net/jtx/mainnet-beta/fundingRate/SOL/history?limit=50&offset=100"

# Get history in a time range
curl "https://gum-api.jup.net/jtx/mainnet-beta/fundingRate/SOL/history?startTime=1709136000&endTime=1709222400"
```

---

### Get User Funding History

Get funding payment history for a trading account.

**Endpoint:** `GET /tradingAccount/{trading_account}/funding`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | number | No | Maximum entries to return (default: 100, max: 500) |
| `offset` | number | No | Offset for pagination (default: 0) |
| `startTime` | number | No | Unix timestamp - filter entries at or after this time |
| `endTime` | number | No | Unix timestamp - filter entries at or before this time |

**Response:**
```json
{
  "entries": [
    {
      "symbol": "SOL",
      "funding": -1.234567,
      "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
      "timestamp": 1709136000
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `entries` | array | Array of user funding entries |
| `entries[].symbol` | string | Market symbol (falls back to market pubkey if symbol unknown) |
| `entries[].funding` | number | Signed funding transfer in USD (positive = received, negative = paid) |
| `entries[].signature` | string | Transaction signature |
| `entries[].timestamp` | number | Unix timestamp (seconds) |

**Notes:**
- Each entry represents a single funding payment (USDC transferred to/from collateral)
- Data sourced from `settle_funding_collateral_log` (the actual USDC settlement events)
- Results are sorted by time descending
- Trading account pubkey is validated (returns 400 if invalid)

**Example:**
```bash
# Get recent funding payments
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/funding

# Get funding payments with pagination
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/funding?limit=50&offset=0"

# Get funding payments in a time range
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/funding?startTime=1709136000&endTime=1709222400"
```

---

## Orders

### Place Order

Place a new order on the orderbook.

**Endpoint:** `POST /order/create`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body (limit order):**
```json
{
  "subaccountId": 0,
  "symbol": "SOL",
  "side": "Bid",
  "orderType": "limit",
  "price": 100.50,
  "quantity": 10.0,
  "reduceOnly": false,
  "timeInForce": "GoodTilCanceled",
  "clientOrderId": 12345,
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Body (isolated-margin order):**
```json
{
  "subaccountId": 0,
  "symbol": "SOL",
  "side": "Bid",
  "orderType": "limit",
  "price": 100.50,
  "quantity": 10.0,
  "collateralAmount": 50000000,
  "tradingAccount": "ISOLATED_TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Body (post-only order):**
```json
{
  "symbol": "SOL",
  "side": "Bid",
  "orderType": "postOnly",
  "price": 100.50,
  "quantity": 10.0,
  "reduceOnly": false,
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Body (market order):**
```json
{
  "symbol": "SOL",
  "side": "Bid",
  "orderType": "market",
  "quantity": 10.0,
  "reduceOnly": false,
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `subaccountId` | number | No | Subaccount index (0-255). Defaults to `0` |
| `symbol` | string | Yes | Market symbol |
| `side` | string | Yes | `"Bid"`/`"Ask"`; lowercase aliases `"bid"`/`"ask"` and `"buy"`/`"sell"` are also accepted |
| `orderType` | string | No | `"limit"` (default), `"postOnly"`, or `"market"` |
| `price` | number | Conditional | `> 0` for `limit` and `postOnly`. For `market`: `0` sweeps at any price; `> 0` acts as a slippage cap |
| `quantity` | number | Yes | Quantity in base units (e.g., 10.0) |
| `reduceOnly` | boolean | No | If true, only reduce existing position |
| `timeInForce` | string/object | No | See below. Defaults to `"ImmediateOrCancel"` for `market` and `"GoodTilCanceled"` for `limit`/`postOnly`. Market orders accept `IOC` or `FOK`; post-only orders reject `IOC` and `FOK` |
| `clientOrderId` | number | No | Optional client-provided order ID |
| `collateralAmount` | number | No | Routes the order to an **isolated-margin** account. When present, this many USDC native units are moved from the cross-margin parent into the isolated subaccount alongside the order; `0` moves no fresh collateral (the order draws on the isolated account's existing position/reservation). Omit it entirely for a plain cross-margin order. The `tradingAccount` must be the matching isolated PDA |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

> **Isolated vs cross margin.** Omitting `collateralAmount` places a cross-margin order (the `place_order` instruction). Including `collateralAmount` (even `0`) places an isolated-margin order (the `place_order_isolated` instruction): the filled portion is backed by the isolated account's own collateral, while any resting remainder is fenced against the collateral reserved for the order — it is *not* backed by the rest of the isolated account's balance. An isolated order whose reservation is too small for its notional is rejected on-chain with `InsufficientMargin`.
>
> Market orders are signaled by `orderType: "market"` and use `price = 0` to sweep the book at any price. A non-zero `price` acts as a slippage cap (bid: max paid; ask: min received). Any unfilled remainder is cancelled. The on-chain `PlaceOrderParams` carries an explicit `OrderType` field, and the matcher special-cases `pricePerLot = 0` as "match anything". Order history surfaces these as `orderType: "market"` with `price = "0"` (sweep) or the formatted cap.
>
> Order-size limits and the mark-price deviation band are enforced on-chain per market. For limit and post-only orders, submitted notional is `baseLots * priceLots * quoteLotSize`. For market sweep orders with `price = 0`, submitted notional is derived from the opposite book's impact price at the requested quantity.
>
> Open caps (`maxOpenOrderBaseLots`, `maxOpenOrderNotionalQuoteNative`) apply whenever `reduceOnly` is `false`; close caps (`maxCloseOrderBaseLots`, `maxCloseOrderNotionalQuoteNative`) apply only when `reduceOnly` is `true`. A non-reduce-only order that flips an existing position is sized against the open caps, not the close caps, because place-order does not consume pending event-heap fills and cannot safely confirm the order will only reduce exposure. Submit closing orders with `reduceOnly: true` to use the close caps.
>
> The per-market open-order-count cap bounds the number of live resting orders a user can hold across both sides. A `place` (or `batch_orders` place) is rejected with custom error code `39` (`OPEN_ORDER_COUNT_LIMIT_EXCEEDED`) only when a residual would rest and raise the user's live-order count past the cap; fully-filled taker orders and non-resting IOC remainders do not consume a slot. `u64::MAX` disables the cap, while `0` forbids new resting orders. Cancellations in the same `batch_orders` transaction run before places, so a `cancel + place` batch that leaves the count unchanged is admitted.
> Post-only orders are signaled by `orderType: "postOnly"` and reject if they would match immediately.

**TimeInForce Values:**

| Value | Aliases | Description |
|-------|---------|-------------|
| `"GoodTilCanceled"` | `"GTC"` | Order remains active until canceled (default) |
| `"ImmediateOrCancel"` | `"IOC"` | Fill immediately, cancel unfilled portion |
| `"FillOrKill"` | `"FOK"` | Fill entirely or cancel completely |
| `{"Seconds": N}` | - | Order expires after N seconds (N must be > 0) |

**Response:**
```json
{
  "symbol": "SOL",
  "clientOrderId": 12345
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/order/create \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "SOL",
    "side": "Bid",
    "orderType": "limit",
    "price": 100.50,
    "quantity": 10.0,
    "timeInForce": "GTC",
    "tradingAccount": "YOUR_TRADING_ACCOUNT",
    "recentBlockhash": "BLOCKHASH",
    "signature": "SIGNATURE"
  }'
```

---

### Cancel Order

Cancel a specific order.

**Endpoint:** `POST /order/cancel`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "subaccountId": 0,
  "isIsolated": false,
  "symbol": "SOL",
  "orderId": "123456789",
  "clientOrderId": null,
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `subaccountId` | number | No | Subaccount index. Defaults to `0` |
| `isIsolated` | boolean | No | Target an isolated-margin account instead of cross-margin. Defaults to `false`. Must match the `tradingAccount`'s margin type or the request is rejected on-chain |
| `symbol` | string | Yes | Market symbol |
| `orderId` | string | No | Order ID to cancel |
| `clientOrderId` | number | No | Client order ID to cancel |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash |
| `signature` | string | Yes | User's signature |

*Note: Provide either `orderId` or `clientOrderId`, not both.*

**Response:**
```json
{
  "success": true
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/order/cancel \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "SOL",
    "orderId": "123456789",
    "tradingAccount": "YOUR_TRADING_ACCOUNT",
    "recentBlockhash": "BLOCKHASH",
    "signature": "SIGNATURE"
  }'
```

---

### Cancel All Orders

Cancel all open orders for a market.

**Endpoint:** `POST /order/cancel-all`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "symbol": "SOL",
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `subaccountId` | number | No | Subaccount index. Defaults to `0` |
| `isIsolated` | boolean | No | Target an isolated-margin account instead of cross-margin. Defaults to `false`. Must match the `tradingAccount`'s margin type or the request is rejected on-chain |
| `symbol` | string | Yes | Market symbol |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash |
| `signature` | string | Yes | User's signature |

**Response:**
```json
{
  "success": true
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/order/cancel-all \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "SOL",
    "tradingAccount": "YOUR_TRADING_ACCOUNT",
    "recentBlockhash": "BLOCKHASH",
    "signature": "SIGNATURE"
  }'
```

---

### Batch Orders

Submit multiple signed place/cancel actions as an API-level batch.

**Endpoint:** `POST /order/batch`

Submit multiple already-signed place/cancel actions in one API request. The API verifies every action first, rejects the whole request if any action is invalid, then publishes each action as a normal single place/cancel message to the sequencer. This is not an on-chain atomic batch.

All actions in a batch must target the same `symbol`; submit separate batch requests for different markets.

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "actions": [
    {
      "method": "place",
      "symbol": "SOL",
      "side": "Bid",
      "orderType": "postOnly",
      "price": 99.0,
      "quantity": 5.0,
      "reduceOnly": false,
      "timeInForce": "GTC",
      "clientOrderId": 1,
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    },
    {
      "method": "cancel",
      "symbol": "SOL",
      "clientOrderId": 1,
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    }
  ]
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `actions` | array | Yes | 1-50 individually signed actions for one symbol |
| `actions[].method` | string | Yes | `"place"` or `"cancel"` |

Each action item uses the same fields and signing rules as `POST /order/create` or `POST /order/cancel`. `cancelAll` is not part of batch v1; use `POST /order/cancel-all` separately.

**Response:**
```json
{
  "success": true,
  "acceptedActions": 2
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/order/batch \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{
    "actions": [
      {"method":"place","symbol":"SOL","side":"Bid","orderType":"postOnly","price":99.0,"quantity":5.0,"tradingAccount":"YOUR_TRADING_ACCOUNT","recentBlockhash":"BLOCKHASH","signature":"SIGNATURE"}
    ]
  }'
```

---

## Trading Account

### Create Trading Account

Create a new trading account. With no query parameters this creates a **cross-margin** subaccount. Pass `is_isolated=true` to instead create an **isolated-margin** account scoped to a single market.

**Endpoint:** `POST /tradingAccount`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `is_isolated` | boolean | No | Create an isolated-margin account instead of cross-margin. Defaults to `false` |
| `symbol` | string | When `is_isolated` | Market the isolated account is bound to (e.g. `ETH`) |
| `subaccount_id` | number | When `is_isolated` | Subaccount index (0-255) of the **existing** cross-margin parent the isolated account settles into |

**Response:**
```json
{
  "tradingAccount": "NEW_TRADING_ACCOUNT_PUBKEY",
  "subaccountId": 0,
  "transactionSignature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `tradingAccount` | string | New trading account public key (the isolated PDA when `is_isolated`) |
| `subaccountId` | number | Assigned subaccount index (0-255) |
| `transactionSignature` | string | Transaction signature |

**Notes:**
- The server pays for the transaction fee.
- **Cross-margin (default):** the server automatically selects the next available subaccount ID (0-255).
- **Isolated-margin (`is_isolated=true`):** the cross-margin parent at `subaccount_id` must already exist — create it first with a plain `POST /tradingAccount`. The isolated account is keyed by `(user, subaccount_id, market)` and is created empty; fund it directly via [`/deposit`](#deposit) using the returned `tradingAccount` address.
- Once created and funded, place orders against it via [Place Order](#place-order): pass the isolated PDA as `tradingAccount`, the matching `subaccountId`, and a `collateralAmount` (use `0` to draw only on the isolated account's own balance) to route to `place_order_isolated`.

**Example (cross-margin):**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

**Example (isolated-margin on ETH, parent subaccount 0):**
```bash
curl -X POST "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount?is_isolated=true&symbol=ETH&subaccount_id=0" \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Rename Subaccount

Rename a subaccount using offchain metadata. This does not modify the on-chain trading account.

**Endpoint:** `POST /tradingAccount/{trading_account}/name`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "name": "Main",
  "blockhash": "RECENT_BLOCKHASH",
  "signature": "USER_SIGNATURE"
}
```

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name for the subaccount. Must not have leading or trailing whitespace. Maximum 64 characters. |
| `blockhash` | string | Yes | Recent blockhash from `GET /blockhash`, included in the signed payload to prevent replay |
| `signature` | string | Yes | User signature over the rename offchain message |

**Signing:**

The `X-PUBKEY` header must match the signing keypair. Use a currently valid recent blockhash and sign a Jupnet offchain-message v0 containing this compact JSON payload:

```json
{"action":"jtx:renameSubaccount","tradingAccount":"TRADING_ACCOUNT_PUBKEY","name":"Main","blockhash":"RECENT_BLOCKHASH"}
```

Rust SDK example (sign the offchain-message payload above with your keypair, then
submit with `gum-perp-sdk`):

```rust
use gum_perp_sdk::{Pubkey, RenameSubAccountRequest, RestClient, TypedSignature};

let user_pubkey: Pubkey = "YOUR_PUBKEY".parse()?;
let trading_account: Pubkey = "TRADING_ACCOUNT_PUBKEY".parse()?;
let client = RestClient::new(user_pubkey);

let blockhash = client.blockhash().await?.blockhash;
let signature_bytes: [u8; 64] = sign_offchain_message_with_your_key(...);
let signature = TypedSignature::from_ed25519(signature_bytes);

let request = RenameSubAccountRequest {
    name: "Main".to_string(),
    blockhash,
    signature,
};
let response = client.rename_subaccount(&trading_account, &request).await?;
```

**Response:**
```json
{
  "pubkey": "TRADING_ACCOUNT_PUBKEY",
  "subaccountId": 0,
  "name": "Main"
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `pubkey` | string | Trading account public key |
| `subaccountId` | number | Subaccount index (0-255) |
| `name` | string | Updated offchain display name |

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/name \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"name":"Main","blockhash":"RECENT_BLOCKHASH","signature":"USER_SIGNATURE"}'
```

---

### List Trading Accounts

List all trading accounts (subaccounts) for a user.

**Endpoint:** `GET /tradingAccount`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |

**Response:**
```json
{
  "exists": true,
  "accounts": [
    {
      "pubkey": "TRADING_ACCOUNT_PUBKEY",
      "subaccountId": 0,
      "name": "Main",
      "data": {
        "freeBalance": "1000.00",
        "tokenBalance": 1000000000,
        "accountValue": "1250.75",
        "totalMaintenanceMargin": "50.25",
        "crossAccountLeverage": "2.5"
      }
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `exists` | boolean | Whether any accounts exist for this user |
| `accounts` | array | Array of trading accounts |
| `accounts[].pubkey` | string | Trading account public key |
| `accounts[].subaccountId` | number | Subaccount index (0-255) |
| `accounts[].name` | string \| null | Offchain display name set via `POST /tradingAccount/{trading_account}/name` |
| `accounts[].data` | object | Account data. The endpoint returns `SERVICE_DEGRADED` instead of partial account entries when required account snapshots are missing |
| `accounts[].data.freeBalance` | string | Available collateral after margin requirements, in quote units |
| `accounts[].data.tokenBalance` | number | Single token balance in smallest units |
| `accounts[].data.accountValue` | string \| null | USD-denominated equity: collateral + unsettled PnL/funding across active positions; `null` when active account risk cannot be priced from the current market cache |
| `accounts[].data.totalMaintenanceMargin` | string \| null | Maintenance margin requirement summed across positions and open orders, in USD; `null` when active account risk cannot be priced from the current market cache |
| `accounts[].data.crossAccountLeverage` | string \| null | Sum of open-position notional / `accountValue`. `0` when `accountValue <= 0` or there are no open positions; `null` when active account risk cannot be priced from the current market cache |

**Notes:**
- Use `/tradingAccount/{trading_account}/positions` for position data
- Use `/tradingAccount/{trading_account}/orders` for open orders

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Get Trading Account Orders

Get all open orders for a trading account across all markets.

**Endpoint:** `GET /tradingAccount/{trading_account}/orders`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Response:**
```json
[
    {
      "symbol": "SOL",
      "orderId": "123456789",
      "side": "Bid",
      "price": "100.50",
      "initialSize": "10.5",
      "remainingSize": "8.0",
      "expiryTimestamp": 0,
      "status": "partially_filled",
      "clientOrderId": 12345,
      "reduceOnly": false
    }
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `orderId` | string | Order ID (u128 as string) |
| `side` | string | `"Bid"` or `"Ask"` |
| `price` | string | Order price |
| `initialSize` | string | Original order size in base units |
| `remainingSize` | string | Remaining unfilled size in base units |
| `expiryTimestamp` | number | Expiry timestamp (0 = no expiry) |
| `status` | string | Order status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"` |
| `clientOrderId` | number \| null | Client-provided order ID (omitted if not set) |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/orders
```

---

### Get Trading Account Positions

Get all perpetual positions for a trading account.

**Endpoint:** `GET /tradingAccount/{trading_account}/positions`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Response:**
```json
[
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
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `side` | string | Position side: `"Bid"` for long or `"Ask"` for short |
| `size` | string | Position size in base units |
| `avgEntryPrice` | string | Average entry price |
| `markPrice` | string | Mark price used for unrealized PnL, margin, and liquidation estimates |
| `unrealizedPnl` | string | Unrealized PnL based on current mark price (USDC) |
| `unrealizedPnlPercent` | string | Unrealized PnL as percentage of entry value |
| `margin` | string | Initial margin requirement for this position (USDC) |
| `liquidationPrice` | string | Estimated liquidation price (cross-margin) |
| `fundingAccrued` | string | Unsettled funding accrued on this position (USDC). Positive = funding received, negative = funding paid |
| `pendingFundingPayable` | string | Pending funding this position still owes (USDC, >= 0). Mirrors the negative side of `fundingAccrued` |
| `pendingFundingReceivable` | string | Pending funding this position is still owed (USDC, >= 0). A pending receivable only — not claimable, transferable, or withdrawable until actually settled |

**Notes:**
- Unrealized PnL is calculated using `markPrice`. In phase one, `markPrice = indexPrice`.
- Liquidation price accounts for cross-margin (shared collateral across all positions)
- Margin is calculated as: notional_value × initial_margin_ratio

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/positions
```

---

### Get Trading Account Balances

Get token balances for a trading account.

**Endpoint:** `GET /tradingAccount/{trading_account}/balances`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Response:**
```json
{
  "freeBalance": "1000.00",
  "tokenBalance": 1000000000
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `freeBalance` | string | Available collateral (after margin requirements), in quote units. The endpoint returns `SERVICE_DEGRADED` when the required account snapshot is missing |
| `tokenBalance` | number | Single token balance in smallest units |

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/balances
```

---

### Get Trading Account Trades

Get trade history for a trading account.

**Endpoint:** `GET /tradingAccount/{trading_account}/trades`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | No | Filter by market symbol (e.g., "SOL") |
| `startTime` | number | No | Start timestamp (Unix seconds). If not provided, returns last 500 trades |
| `endTime` | number | No | End timestamp (Unix seconds). Defaults to now |

**Response:**
```json
[
    {
      "symbol": "SOL",
      "side": "Bid",
      "price": "100.50",
      "size": "10.5",
    "txid": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
    "time": 1702656000,
    "clientOrderId": 123456,
    "fee": "0.050250",
    "realizedPnl": "5.25"
  }
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `side` | string | Trade side from user's perspective: `"Bid"` (buy) or `"Ask"` (sell) |
| `price` | string | Trade price |
| `size` | string | Trade size in base units |
| `txid` | string | Transaction signature |
| `time` | number | Unix timestamp (seconds) |
| `clientOrderId` | number \| null | Client-provided order ID for this trading account's side of the fill. Null if the order was submitted without a client order ID |
| `fee` | string | Fee charged for this trade in USDC (negative for maker rebates) |
| `realizedPnl` | string \| null | Realized PnL excluding fees. Null if the trade opened or increased a position. Populated for both maker and taker fills that reduce an existing position |

**Example:**
```bash
# Get last 500 trades
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/trades

# Get trades for specific market
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/trades?symbol=SOL"

# Get trades in time range
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/trades?startTime=1702656000&endTime=1702742400"
```

---

### Get Order History

Get historical orders (including filled, cancelled, and open orders) for a trading account.
Liquidation attempts that execute zero base size are omitted.

**Endpoint:** `GET /tradingAccount/{trading_account}/orderHistory`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | No | Filter by market symbol (e.g., "SOL") |
| `status` | string | No | Filter by status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"` |
| `limit` | number | No | Maximum orders to return (default: 100, max: 500) |
| `offset` | number | No | Offset for pagination (default: 0) |
| `startTime` | number | No | Unix timestamp - filter orders created at or after this time |
| `endTime` | number | No | Unix timestamp - filter orders created at or before this time |

**Response:**
```json
{
  "orders": [
    {
      "orderId": "123456789012345678901234567890",
      "clientOrderId": 12345,
      "symbol": "SOL",
      "side": "Bid",
      "price": "100.50",
      "originalSize": "10.5",
      "filledSize": "5.25",
      "realizedPnl": null,
      "status": "partially_filled",
      "orderType": "limit",
      "timeInForce": "gtc",
      "reduceOnly": true,
      "createdAt": 1702656000,
      "updatedAt": 1702656100,
      "transactions": [
        {
          "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
          "type": "submit",
          "timestamp": 1702656000
        },
        {
          "signature": "6fylt5VtGw9Q9OKeUREqZ2waKrLrKve248ex3O0e",
          "type": "fill",
          "timestamp": 1702656100,
          "price": "100.50",
          "size": "5.25",
          "fee": "0.002625",
          "role": "maker"
        }
      ]
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `orders` | array | Array of order history entries |
| `orders[].orderId` | string | Order ID (u128 as string) |
| `orders[].clientOrderId` | number \| null | Client-provided order ID, or `null` if not set |
| `orders[].symbol` | string | Market symbol (e.g., "SOL") |
| `orders[].side` | string | Order side: `"Bid"` or `"Ask"` |
| `orders[].price` | string | Parent order price. Once an order has fill transactions, this is the base-size-weighted average execution price; unfilled orders show the submitted limit price. |
| `orders[].originalSize` | string | Original order size in base units |
| `orders[].filledSize` | string | Filled size in base units |
| `orders[].realizedPnl` | string \| null | Realized PnL in USDC summed from this order's closing fill transactions; `null` when the order has no realized PnL. Fees are exposed separately on fill transactions and are not netted into this value. |
| `orders[].status` | string | Order status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"` |
| `orders[].orderType` | string | Order type: `"limit"`, `"market"`, `"postOnly"`, `"liquidation"`, or `"adl"` |
| `orders[].timeInForce` | string | Time in force: `"gtc"`, `"gtd"`, `"ioc"`, `"fok"`, `"liquidation"`, or `"adl"` |
| `orders[].reduceOnly` | boolean | Whether the order was submitted as reduce-only |
| `orders[].createdAt` | number | Unix timestamp when order was created |
| `orders[].updatedAt` | number | Unix timestamp of last update |
| `orders[].transactions` | array | List of transactions related to this order |
| `orders[].transactions[].signature` | string | Transaction signature |
| `orders[].transactions[].type` | string | Transaction type: `"submit"`, `"fill"`, `"cancel"` |
| `orders[].transactions[].timestamp` | number | Unix timestamp |
| `orders[].transactions[].price` | string? | Fill price (only for `"fill"` transactions) |
| `orders[].transactions[].size` | string? | Fill size in base units (only for `"fill"` transactions) |
| `orders[].transactions[].fee` | string? | Normal maker/taker trading fee in USDC (only for `"fill"` transactions; excludes liquidation fee) |
| `orders[].transactions[].realizedPnl` | string? | Realized PnL in USDC for this closing fill transaction. Omitted for submits, cancels, and fills with no realized PnL. Fees are exposed separately and are not netted into this value. |
| `orders[].transactions[].liquidationFee` | string? | Liquidation fee in USDC reported by `LiquidationLog` (only for liquidation fill transactions). This is the accrued/maximum fee; the actual fee deducted on-chain can be lower or zero when collateral is capped or the account enters bad debt. |
| `orders[].transactions[].role` | string? | `"maker"` or `"taker"` (only for `"fill"` transactions) |

**Example:**
```bash
# Get all order history
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory

# Get filled orders only
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?status=filled"

# Get orders for specific market with pagination
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?symbol=SOL&limit=50&offset=100"

# Get orders in a time range
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?startTime=1702656000&endTime=1702742400"
```

---

### Get Transaction History

Get unified transaction history (deposits, withdrawals, settle PnL, settle funding) for a trading account.

**Endpoint:** `GET /tradingAccount/{trading_account}/transactionHistory`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `transactionType` | string | No | Filter by type: `"deposit"`, `"withdraw"`, `"settle_pnl"`, `"settle_funding"` |
| `limit` | number | No | Maximum entries to return (default: 100, max: 500) |
| `offset` | number | No | Offset for pagination (default: 0) |
| `startTime` | number | No | Unix timestamp - filter transactions at or after this time |
| `endTime` | number | No | Unix timestamp - filter transactions at or before this time |

**Response:**
```json
{
  "transactions": [
    {
      "transactionType": "deposit",
      "amount": "1000.000000",
      "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
      "timestamp": 1702656000
    },
    {
      "transactionType": "settle_pnl",
      "amount": "50.250000",
      "symbol": "SOL",
      "signature": "6fylt5VtGw9Q9OKeUREqZ2waKrLrKve248ex3O0e",
      "timestamp": 1702656100
    },
    {
      "transactionType": "withdraw",
      "amount": "-500.000000",
      "signature": "7gzmu6WuHx0R0PLfVSFr3wbLsMs359fx4P1f",
      "timestamp": 1702656200
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `transactions` | array | Array of transaction history entries |
| `transactions[].transactionType` | string | Transaction type: `"deposit"`, `"withdraw"`, `"settle_pnl"`, `"settle_funding"` |
| `transactions[].amount` | string | Signed amount in USDC (positive = credit, negative = debit) |
| `transactions[].symbol` | string \| null | Market symbol (present for `settle_pnl` and `settle_funding`, absent for `deposit`/`withdraw`) |
| `transactions[].signature` | string | Transaction signature |
| `transactions[].timestamp` | number | Unix timestamp (seconds) |

**Notes:**
- Amounts use fixed-point formatting with 6 decimal places (USDC decimals)
- Withdrawals are shown as negative amounts
- Settle funding amounts are negated (positive = funding received, negative = funding paid)
- Results are sorted by timestamp descending

**Example:**
```bash
# Get all transaction history
curl https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory

# Get only deposits
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?transactionType=deposit"

# Get transactions with pagination
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?limit=50&offset=100"

# Get transactions in a time range
curl "https://gum-api.jup.net/jtx/mainnet-beta/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?startTime=1702656000&endTime=1702742400"
```

---

## Deposit

### Build Deposit Transaction

Build an unsigned deposit transaction for the user to sign.

**Endpoint:** `GET /deposit`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Depositor's public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `amount` | number | Yes | Deposit amount in smallest units (e.g., 1000000 = 1 USDC) |
| `trading_account` | string | Yes | Target trading account public key (cross or isolated) |

**Response:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_TRANSACTION"
}
```

**Notes:**
- The returned transaction is unsigned
- User must sign with their private key
- Server is the fee payer
- `trading_account` may be an isolated-margin account — deposit credits its collateral directly, no separate transfer from the cross parent is required

**Example:**
```bash
curl "https://gum-api.jup.net/jtx/mainnet-beta/deposit?amount=1000000&trading_account=YOUR_TRADING_ACCOUNT" \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Submit Deposit

Submit a signed deposit transaction.

**Endpoint:** `POST /deposit`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Depositor's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_SIGNED_TRANSACTION"
}
```

**Response:**
```json
{
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/deposit \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"serializedTransaction": "BASE64_SIGNED_TX"}'
```

---

## Withdraw

### Build Withdraw Transaction

Build an unsigned withdraw transaction for the user to sign.

**Endpoint:** `GET /withdraw`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Withdrawer's public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `amount` | number | Yes | Withdraw amount in smallest units (e.g., 1000000 = 1 USDC) |
| `trading_account` | string | Yes | Source trading account public key (cross or isolated) |

**Response:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_TRANSACTION"
}
```

**Notes:**
- The returned transaction is unsigned
- User must sign with their private key
- Server is the fee payer
- Amount must be greater than 0
- User must own the trading account
- Withdrawal will fail on-chain if it would bring the account below margin requirements
- `trading_account` may be an isolated-margin account; the API supplies its cross-margin parent, and any reducing fill consumed during the withdraw settles freed equity into the parent before payout

**Example:**
```bash
curl "https://gum-api.jup.net/jtx/mainnet-beta/withdraw?amount=1000000&trading_account=YOUR_TRADING_ACCOUNT" \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Submit Withdraw

Submit a signed withdraw transaction.

**Endpoint:** `POST /withdraw`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Withdrawer's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_SIGNED_TRANSACTION"
}
```

**Response:**
```json
{
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/withdraw \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"serializedTransaction": "BASE64_SIGNED_TX"}'
```

---

## Transfer

### Build Transfer Transaction

Build an unsigned collateral-transfer transaction for the user to sign. This
moves collateral between two trading accounts owned by the same `X-PUBKEY` —
either between two cross-margin subaccounts, or between a cross-margin
subaccount and one of its isolated-margin accounts (in either direction). The
transfer direction is inferred from the account types; no tokens leave the
vault.

**Endpoint:** `GET /transfer`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Owner of both trading accounts |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `amount` | number | Yes | Transfer amount in smallest units (e.g., 1000000 = 1 USDC) |
| `source_trading_account` | string | Yes | Source trading account public key (cross-margin subaccount or isolated-margin account) |
| `destination_trading_account` | string | Yes | Destination trading account public key (cross-margin subaccount or isolated-margin account) |

**Response:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_TRANSACTION"
}
```

**Notes:**
- The returned transaction is unsigned
- User must sign with their private key
- Server is the fee payer
- Amount must be greater than 0
- Both accounts must be owned by `X-PUBKEY`; cross-to-cross transfers must use two different subaccounts
- Cross↔isolated transfers must pair an isolated account with its parent cross-margin subaccount; transfers between two isolated accounts are not supported
- Transfer will fail on-chain if it would bring the source account below margin requirements
- The transaction embeds the source account's markets and open orders as they were at build time; if positions or open orders change before submission, the transfer can fail on-chain — rebuild via `GET /transfer` and resubmit

**Example:**
```bash
curl "https://gum-api.jup.net/jtx/mainnet-beta/transfer?amount=1000000&source_trading_account=SOURCE_TRADING_ACCOUNT&destination_trading_account=DESTINATION_TRADING_ACCOUNT" \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Submit Transfer

Submit a signed transfer transaction.

**Endpoint:** `POST /transfer`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | Owner of both trading accounts |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "serializedTransaction": "BASE64_ENCODED_SIGNED_TRANSACTION"
}
```

**Response:**
```json
{
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

**Example:**
```bash
curl -X POST https://gum-api.jup.net/jtx/mainnet-beta/transfer \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"serializedTransaction": "BASE64_SIGNED_TX"}'
```

---

## Blockhash

### Get Recent Blockhash

Get the current recent blockhash for signing transactions.

**Endpoint:** `GET /blockhash`

**Response:**
```json
{
  "blockhash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
}
```

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/blockhash
```

---

## Health

### Health Check

Check the health status of the API service and its dependencies.

**Endpoint:** `GET /health`

**Response:**
```json
{
  "status": "healthy",
  "trading_available": true,
  "ohlcv_available": true,
  "degraded_services": []
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Overall status: `"healthy"`, `"degraded"`, or `"unhealthy"` |
| `trading_available` | boolean | Whether trading functionality is available (requires a fresh cached blockhash) |
| `ohlcv_available` | boolean | Whether OHLCV/candlestick data is available (requires the historical data store) |
| `degraded_services` | array | List of unavailable services: `"redis"`, `"clickhouse"`, `"blockhash"`, `"postgres"` |

**Status Codes:**

| Status Code | Condition |
|-------------|-----------|
| 200 OK | All services healthy or degraded (core functionality available) |
| 503 Service Unavailable | Critical services unavailable (cache down or cached blockhash stale) |

**Status Meanings:**

| Status | Description |
|--------|-------------|
| `healthy` | All services operational |
| `degraded` | Core trading works, but optional services (historical data store) unavailable |
| `unhealthy` | Critical services (cache) unavailable, trading not possible |

**Example:**
```bash
curl https://gum-api.jup.net/jtx/mainnet-beta/health
```

---

## WebSocket

### WebSocket Endpoint

Connect to the WebSocket endpoint for real-time updates.

**Endpoint:** `GET /ws` (WebSocket upgrade)

See [WEBSOCKET_API.md](./WEBSOCKET_API.md) for WebSocket documentation.

---

## Error Responses

All endpoints return errors in the following format:

```json
{
  "success": false,
  "error": "Error message description",
  "error_code": "BAD_REQUEST",
  "retryable": false
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | boolean | Always `false` for error responses |
| `error` | string | Human-readable error message |
| `error_code` | string | Machine-readable error code (see below) |
| `retryable` | boolean | `true` if the error is transient and client should retry |

**Error Codes:**

| Error Code | HTTP Status | Retryable | Description |
|------------|-------------|-----------|-------------|
| `BAD_REQUEST` | 400 | No | Client provided invalid input |
| `NOT_FOUND` | 404 | No | Requested resource not found |
| `RPC_UNAVAILABLE` | 503 | Yes | Solana RPC node connection unavailable |
| `SERVICE_DEGRADED` | 503 | Yes | A backend dependency is temporarily unavailable |
| `INVALID_TRANSACTION` | 400 | No | Transaction contains invalid data |
| `TRANSACTION_TIMEOUT` | 504 | Yes | Transaction confirmation timed out |
| `TRANSACTION_FAILED` | 500 | No | Transaction failed on-chain |
| `INTERNAL` | 500 | No | Internal server error |

**Retry-After Header:**

For retryable errors (503, 504), the response includes a `Retry-After` header indicating how long to wait before retrying:

```
Retry-After: 5
```

**Client Retry Logic:**

```javascript
async function fetchWithRetry(url, options, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    const response = await fetch(url, options);
    const data = await response.json();

    if (response.ok) return data;

    if (data.retryable && i < maxRetries - 1) {
      const retryAfter = response.headers.get('Retry-After') || 5;
      await new Promise(r => setTimeout(r, retryAfter * 1000));
      continue;
    }

    throw new Error(data.error);
  }
}
```

**Common HTTP Status Codes:**

| Code | Description |
|------|-------------|
| 400 | Bad Request - Invalid parameters or transaction |
| 404 | Not Found - Resource not found |
| 500 | Internal Server Error - Non-transient failure |
| 503 | Service Unavailable - Transient, retry recommended |
| 504 | Gateway Timeout - Transient, retry recommended |

---

## Summary

| Method | Path | Description |
|--------|------|-------------|
| GET | `/markets` | List all markets |
| GET | `/market/{symbol}` | Get market info |
| GET | `/orderbook/{symbol}` | Get orderbook |
| GET | `/trades/{symbol}` | Get recent trades |
| GET | `/ohlcv/{symbol}` | Get OHLCV candles |
| GET | `/fundingRates` | Get all current funding rates |
| GET | `/fundingRate/{symbol}` | Get current funding rate |
| GET | `/fundingRate/{symbol}/history` | Get funding rate history |
| GET | `/tradingAccount/{trading_account}/funding` | Get user funding history |
| POST | `/order/create` | Place new order |
| POST | `/order/cancel` | Cancel order |
| POST | `/order/cancel-all` | Cancel all orders |
| POST | `/order/batch` | Submit signed place/cancel actions as an API-level batch |
| GET | `/tradingAccount` | List user's accounts |
| POST | `/tradingAccount` | Create trading account |
| POST | `/tradingAccount/{trading_account}/name` | Rename subaccount |
| GET | `/tradingAccount/{trading_account}/orders` | Get all open orders |
| GET | `/tradingAccount/{trading_account}/positions` | Get perpetual positions |
| GET | `/tradingAccount/{trading_account}/balances` | Get token balances |
| GET | `/tradingAccount/{trading_account}/trades` | Get trade history |
| GET | `/tradingAccount/{trading_account}/orderHistory` | Get order history |
| GET | `/tradingAccount/{trading_account}/transactionHistory` | Get transaction history |
| GET | `/deposit` | Build deposit transaction |
| POST | `/deposit` | Submit signed deposit |
| GET | `/withdraw` | Build withdraw transaction |
| POST | `/withdraw` | Submit signed withdraw |
| GET | `/transfer` | Build cross-subaccount transfer transaction |
| POST | `/transfer` | Submit signed transfer |
| GET | `/blockhash` | Get recent blockhash |
| GET | `/health` | Health check |
| GET | `/ws` | WebSocket endpoint |
