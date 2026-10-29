# REST API Documentation

## Base URL

```
https://api.gum.ag/perps
```

## Response Headers

Every response carries the server machine timestamp:

```
x-machine-timestamp: 1756339200123
```

The value is milliseconds since the Unix epoch, read from the server clock when the response is sent. Use it for latency measurement and clock-skew estimation.

## Authentication

Endpoints whose **Headers** table requires `X-PUBKEY` use it to identify the user. Public market and account-by-address reads do not require it. The header alone is not proof of ownership; signed write endpoints verify the signature as described below.

```
X-PUBKEY: YOUR_PUBLIC_KEY
```

### Signing Trading Actions

Trading write endpoints (`/order/create`, `/order/cancel`, `/order/cancel-all`, and `/order/batch`) require signed action payloads. The request body includes the action fields plus:

| Field | Description |
|-------|-------------|
| `tradingAccount` | The user's **cross-margin subaccount** the action is scoped to (`[cross_margin, user, subaccountId]`). Isolated-margin actions send this same value — the isolated account is selected by `isIsolated` + `subaccountId` + `symbol`, never by passing its address. In both modes the value must be the cross-margin PDA for the signed `subaccountId`; anything else is rejected with `400 BAD_REQUEST` |
| `recentBlockhash` | Recent blockhash used when signing |
| `signature` | User signature over the serialized JTX action |

The `X-PUBKEY` header must be the public key for the signing keypair. The API reconstructs the same action from the JSON body, serializes it with the Jupnet action format, and verifies `signature` against `X-PUBKEY`.

> **Every signed action is single-use.** Once the API has accepted an action it is queued for the sequencer and, within seconds, on-chain — resubmitting the same signed body does nothing more. A resend that reaches the same API instance while the signature is still tracked (about 45 s, the blockhash validity window) is rejected up front with `409 DUPLICATE_SIGNATURE`, before signature verification; a resend that slips past that check is dropped silently by the sequencer. Do not retry an accepted action by resending it: watch the outcome via the `tradingAccountEvents` / `openOrders` WebSocket streams, and if you do need to act again, sign a fresh action with a new blockhash. Only a request that was *not* accepted (a 4xx/5xx other than `409`) can be resubmitted unchanged.

> **Order action rate limit.** Signed trading actions (`POST /order/create`, `/order/cancel`, `/order/cancel-all`, `/order/batch`, and the WebSocket `post` actions) are limited per signing wallet, per API instance: 100 actions/s with bursts of 200 by default, or 3,000/s with bursts of 6,000 on a market where the wallet holds an active whitelist entry. Every action counts, cancels included, and a batch counts as one per action. Only actions with a valid signature count, so another wallet can't use up yours. Over the limit, the request fails with `429 RATE_LIMITED` (`retryable: true`, `Retry-After: 1`).

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
    is_isolated: false,
};

let signed = order.sign_request_with(trading_account, &market, blockhash, &signer)?;
let response = client.place_order(&signed).await?;
```

For an isolated-margin order, set `is_isolated: true` and `collateral_amount: Some(usdc_native)`;
`trading_account` stays the cross-margin subaccount.

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

The API does not validate `recentBlockhash` freshness on order endpoints; the value is part of the signed payload and is passed through as-is. Fetch it from `GET /blockhash` or the WebSocket `blockhash` stream when signing.

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
    "limitOrderDeviationBps": 500,
    "orderLimitConfig": {
      "limit": {
        "minOrderQty": 0.01,
        "minOrderNotional": 5.0,
        "maxOpenOrderQty": 1000.0,
        "maxCloseOrderQty": 2000.0,
        "maxOpenOrderNotional": 100000.0,
        "maxCloseOrderNotional": 200000.0
      },
      "market": {
        "minOrderQty": 0.01,
        "minOrderNotional": 5.0,
        "maxOpenOrderQty": 1000.0,
        "maxCloseOrderQty": 2000.0,
        "maxOpenOrderNotional": 100000.0,
        "maxCloseOrderNotional": 200000.0
      },
      "postOnly": {
        "minOrderQty": 0.01,
        "minOrderNotional": 5.0,
        "maxOpenOrderQty": 1000.0,
        "maxCloseOrderQty": 2000.0,
        "maxOpenOrderNotional": 100000.0,
        "maxCloseOrderNotional": 200000.0
      }
    },
    "maxAccountPositionNotional": 1000000.0,
    "maxMarketOpenInterest": 5000000.0,
    "indexPrice": 102.625,
    "markPrice": 102.625,
    "bestBid": 102.50,
    "bestAsk": 102.75,
    "midPrice": 102.625,
    "initialMarginRatioBps": 1000,
    "maintenanceMarginRatioBps": 500,
    "maxLeverage": 10,
    "isAdlEnabled": false,
    "timestamp": 1702656000,
    "lastUpdatedMs": 100,
    "riskTiers": [
      {
        "notionalCap": 100000.0,
        "initialMarginRatioBps": 1000,
        "maintenanceMarginRatioBps": 500,
        "maxLeverage": 10,
        "maintenanceAmount": 0.0
      },
      {
        "notionalCap": 500000.0,
        "initialMarginRatioBps": 2000,
        "maintenanceMarginRatioBps": 1000,
        "maxLeverage": 5,
        "maintenanceAmount": 5000.0
      }
    ],
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
| `limitOrderDeviationBps` | number | Always present. Market limit-order price deviation in basis points; `4294967295` (`u32::MAX`) disables this check. |
| `orderLimitConfig` | object | Always present, with `limit`, `market`, and `postOnly` groups containing the six fields below. |
| `maxAccountPositionNotional` | number | Always present. Market-level per-account position notional cap in USD (quote-native ÷ `1_000_000`); whitelist overrides are not reflected. Unrestricted caps are `u64::MAX` converted the same way. |
| `maxMarketOpenInterest` | number | Always present. Total market open-interest cap in USD, compared against `max(long, short)` open interest, not their sum. Unrestricted caps are `u64::MAX` converted the same way. |
| `indexPrice` | number | Current index/oracle valuation price |
| `markPrice` | number | Current risk price used for margin, liquidation estimates, unrealized PnL, and withdrawal health. In phase one this equals `indexPrice`. |
| `bestBid` | number | Best bid price (0 if no bids) |
| `bestAsk` | number | Best ask price (0 if no asks) |
| `midPrice` | number | Mid price (average of best bid and ask, 0 if either side is empty) |
| `initialMarginRatioBps` | number | Initial margin requirement in basis points (e.g., 1000 = 10%). On a tiered market this is the first `riskTiers` band. |
| `maintenanceMarginRatioBps` | number | Maintenance margin requirement in basis points (e.g., 500 = 5%). On a tiered market this is the first `riskTiers` band. |
| `maxLeverage` | number | Largest user-selectable leverage for `POST /leverage`. On a tiered market this is the first `riskTiers` band's `maxLeverage`; whitelisted accounts are bounded by their own table instead. |
| `isAdlEnabled` | boolean | Whether auto-deleveraging is enabled. When `true`, a liquidation whose close order cannot fill at the bankruptcy price closes the rest against opposing positions, which then show `closeReason: "adl"` in position history. When `false`, the position stays in liquidation and the close is retried until the book fills it. |
| `timestamp` | number | Unix timestamp of last update |
| `lastUpdatedMs` | number | Milliseconds since last update |
| `riskTiers` | array | Notional bands the market's default table margins on, ascending. Empty when the market applies `initialMarginRatioBps` at every size. |
| `openInterest` | number | Total open interest notional value in quote currency (base quantity × price) |
| `openInterestBase` | number | Total open interest in base units (sum of all long positions) |
| `high24h` | number | 24-hour high price |
| `low24h` | number | 24-hour low price |
| `volume24h` | number | 24-hour trading volume in base units |
| `lastTradePrice` | number | Last trade price (0 if no trades) |
| `priceChange24h` | number | Price change in last 24 hours |
| `priceChangePercent24h` | number | Price change percentage in last 24 hours |
| `statsLastUpdatedMs` | number | Timestamp when stats were last computed (Unix ms) |

**`orderLimitConfig.limit`, `.market`, and `.postOnly` Fields:**

All six fields are always populated as JSON numbers for each order type.

| Field | Type | Description |
|-------|------|-------------|
| `minOrderQty` | number | Minimum order quantity in base-token units |
| `minOrderNotional` | number | Minimum order notional in USD |
| `maxOpenOrderQty` | number | Maximum opening order quantity in base-token units |
| `maxCloseOrderQty` | number | Maximum closing order quantity in base-token units |
| `maxOpenOrderNotional` | number | Maximum opening order notional in USD |
| `maxCloseOrderNotional` | number | Maximum closing order notional in USD |

Quantities are raw base lots × `stepSize`; notionals are raw quote-native amounts
÷ `1_000_000` (not quote lots × `quoteLotSize`). Unrestricted on-chain maxima
(`u64::MAX`) use the same conversion and remain large finite numbers, not `null`
or infinity; these floating-point values are not lossless encodings of raw integers.

Both market endpoints expose the same cached market-level configuration, without
an extra request-time RPC fetch. These values are not account-specific effective
limits or a guarantee of on-chain admission. The limit-order deviation field does
not describe the separately configured market-order or post-only price bands.

**`riskTiers[]` Fields:**

A position's margin comes from the first band its notional fits under, testing
`notional < notionalCap`. A notional landing exactly on a cap therefore belongs
to the band above — except on the last band, whose cap is inclusive. The first
band starts at 0, each later one at the previous band's cap.

These bands are the market's default table. An account whitelisted onto another
table margins on different bands, which these endpoints do not return.

| Field | Type | Description |
|-------|------|-------------|
| `notionalCap` | number | Where the band ends |
| `initialMarginRatioBps` | number | Initial margin requirement within the band |
| `maintenanceMarginRatioBps` | number | Maintenance margin requirement within the band |
| `maxLeverage` | number | Largest selectable leverage within the band |
| `maintenanceAmount` | number | Deduction keeping maintenance continuous with the band below |

Maintenance margin is `notional × maintenanceMarginRatioBps / 10000 - maintenanceAmount`,
floored at 0. A notional above the last `notionalCap` uses the last band.

**Example:**
```bash
curl https://api.gum.ag/perps/markets
```

---

### Get Market

Get information for a specific market. Response fields and units are identical to
an entry in `GET /markets`, including all trading-limit groups.

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
  "limitOrderDeviationBps": 500,
  "orderLimitConfig": {
    "limit": {
      "minOrderQty": 0.01,
      "minOrderNotional": 5.0,
      "maxOpenOrderQty": 1000.0,
      "maxCloseOrderQty": 2000.0,
      "maxOpenOrderNotional": 100000.0,
      "maxCloseOrderNotional": 200000.0
    },
    "market": {
      "minOrderQty": 0.01,
      "minOrderNotional": 5.0,
      "maxOpenOrderQty": 1000.0,
      "maxCloseOrderQty": 2000.0,
      "maxOpenOrderNotional": 100000.0,
      "maxCloseOrderNotional": 200000.0
    },
    "postOnly": {
      "minOrderQty": 0.01,
      "minOrderNotional": 5.0,
      "maxOpenOrderQty": 1000.0,
      "maxCloseOrderQty": 2000.0,
      "maxOpenOrderNotional": 100000.0,
      "maxCloseOrderNotional": 200000.0
    }
  },
  "maxAccountPositionNotional": 1000000.0,
  "maxMarketOpenInterest": 5000000.0,
  "indexPrice": 102.625,
  "markPrice": 102.625,
  "bestBid": 102.50,
  "bestAsk": 102.75,
  "midPrice": 102.625,
  "initialMarginRatioBps": 1000,
  "maintenanceMarginRatioBps": 500,
  "maxLeverage": 10,
  "isAdlEnabled": false,
  "timestamp": 1702656000,
  "lastUpdatedMs": 100,
  "riskTiers": [
    {
      "notionalCap": 100000.0,
      "initialMarginRatioBps": 1000,
      "maintenanceMarginRatioBps": 500,
      "maxLeverage": 10,
      "maintenanceAmount": 0.0
    },
    {
      "notionalCap": 500000.0,
      "initialMarginRatioBps": 2000,
      "maintenanceMarginRatioBps": 1000,
      "maxLeverage": 5,
      "maintenanceAmount": 5000.0
    }
  ],
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
curl https://api.gum.ag/perps/market/SOL
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
curl https://api.gum.ag/perps/orderbook/SOL

# Full orderbook (up to 500 levels per side)
curl https://api.gum.ag/perps/orderbook/SOL?depth=500
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
curl https://api.gum.ag/perps/trades/SOL
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
curl "https://api.gum.ag/perps/ohlcv/SOL?interval=1h&from=1702656000&to=1702742400"
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
| `fundingRates[].rateBps1h` | number | `fundingRate` scaled from `fundingIntervalSeconds` to one hour, in basis points |
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

**Example:**
```bash
curl https://api.gum.ag/perps/fundingRates
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
curl https://api.gum.ag/perps/fundingRate/SOL
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
- `rateBps1h` is derived from the exact on-chain accrual using the fractional funding boundary: `effectiveIndex = distributed_index * 10_000 + frac_distributed`, then `(effectiveIndex(t) - effectiveIndex(t-1h)) * 10_000 / (10_000 * base_lot_value)` (the first historical entry, with no prior distribution row, is excluded)
- Results are sorted by time descending

**Example:**
```bash
# Get latest funding rate history
curl https://api.gum.ag/perps/fundingRate/SOL/history

# Get history with pagination
curl "https://api.gum.ag/perps/fundingRate/SOL/history?limit=50&offset=100"

# Get history in a time range
curl "https://api.gum.ag/perps/fundingRate/SOL/history?startTime=1709136000&endTime=1709222400"
```

---

### Get User Funding History

Get funding payment history for a trading account.

> **Family aggregation.** Querying a cross-margin subaccount also returns the rows of its isolated-margin accounts (same user and `subaccountId`). Querying an isolated account directly returns only its own rows.

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
      "side": "Bid",
      "size": "12.5",
      "fundingRate": "0.000125",
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
| `entries[].side` | string \| null | Position side when the payment settled: `"Bid"` = long, `"Ask"` = short. `null` when the position had already been closed (the payment accrued before the close) or the entry predates position logging |
| `entries[].size` | string \| null | Absolute position size in base units when the payment settled (`"0"` for an already-closed position). `null` when the entry predates position logging or the market is unknown |
| `entries[].fundingRate` | string \| null | Realized funding rate of the hourly distribution the payment settled, as a decimal per hour (multiply by 100 for a percentage): the hour's change in the market's distributed funding index divided by the oracle value of one base lot at the top-of-hour funding update. Equals `rateBps1h` / 10,000 on `/fundingRate/{symbol}/history` for that hour. Positive = longs pay. `null` when the distribution is not indexed |
| `entries[].signature` | string | Transaction signature |
| `entries[].timestamp` | number | Unix timestamp (seconds) |

**Notes:**
- Each entry represents a single funding payment (USDC transferred to/from collateral)
- Data sourced from `settle_funding_collateral_log` (the actual USDC settlement events)
- A full position close rounds the last fraction of a quote-native unit of funding up to a whole unit ($0.000001) against the closer, which settles later against the closed position. These dust settlements (closed position, at most $0.000001) are omitted; real payments on a closed position are still listed
- `side` and `size` are snapshots taken by the program at settlement. For an hourly payment on a position unchanged since the previous one, `|funding| = size × price × |fundingRate|` to the nearest $0.000001, where price is the oracle price at the top-of-hour funding update. It differs when the position changed between the distribution and the settlement (a trade first accrues funding at the old size), when the payment spans several distributions, or when a receiver was paid only part of what accrued
- Results are sorted by time descending
- Trading account pubkey is validated (returns 400 if invalid)

**Example:**
```bash
# Get recent funding payments
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/funding

# Get funding payments with pagination
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/funding?limit=50&offset=0"

# Get funding payments in a time range
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/funding?startTime=1709136000&endTime=1709222400"
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
  "clientOrderId": "12345",
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
  "isIsolated": true,
  "collateralAmount": 50000000,
  "tradingAccount": "CROSS_MARGIN_TRADING_ACCOUNT_PUBKEY",
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
| `subaccountId` | number | No | Subaccount index (0-255). Defaults to `0`; cross-margin placement rejects reserved ids `200..=255`, while isolated placement remains allowed |
| `symbol` | string | Yes | Market symbol |
| `side` | string | Yes | `"Bid"`/`"Ask"`; lowercase aliases `"bid"`/`"ask"` and `"buy"`/`"sell"` are also accepted |
| `orderType` | string | No | `"limit"` (default), `"postOnly"`, or `"market"` |
| `price` | number | Conditional | `> 0` for `limit` and `postOnly`. For `market`: `0` sweeps at any price; `> 0` acts as a slippage cap |
| `quantity` | number | Yes | Quantity in base units (e.g., 10.0) |
| `reduceOnly` | boolean | No | If true, only reduce existing position |
| `timeInForce` | string/object | No | See below. Defaults to `"ImmediateOrCancel"` for `market` and `"GoodTilCanceled"` for `limit`/`postOnly`. Market orders accept `IOC` or `FOK`; post-only orders reject `IOC` and `FOK` |
| `clientOrderId` | string \| number | No | Optional client-provided order ID. Send it as a decimal string: IDs above 2^53 lose precision when a JS client parses a JSON number. A number is still accepted |
| `isIsolated` | boolean | No | Route the order to an **isolated-margin** account (the `place_order_isolated` instruction). Must be paired with `collateralAmount`; defaults to `false` (cross-margin) |
| `collateralAmount` | number | When `isIsolated` | USDC native units moved from the cross-margin parent into the isolated subaccount alongside the order; `0` moves no fresh collateral (the order draws on the isolated account's existing position/reservation). Must be paired with `isIsolated`, and must be `0` when `reduceOnly` is set — a reduce-only order never draws collateral, so a nonzero budget is rejected with `400 BAD_REQUEST` (the chain enforces the same rule). The isolated account does **not** need to exist yet — the API creates it automatically before accepting the order (see below) |
| `tradingAccount` | string | Yes | The user's cross-margin subaccount, for isolated orders too |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

> **Isolated vs cross margin.** `isIsolated: true` with a `collateralAmount` (even `0`) places an isolated-margin order (the `place_order_isolated` instruction): the filled portion is backed by the isolated account's own collateral, while any resting remainder is fenced against the collateral reserved for the order — it is *not* backed by the rest of the isolated account's balance. An isolated order whose reservation is too small — the notional at the worse of its limit and the mark, plus what an adverse limit books on the fill — is rejected on-chain with `InsufficientMargin`. Omitting both fields places a plain cross-margin order (the `place_order` instruction); providing only one of the two is rejected with `400 BAD_REQUEST` ("isIsolated and collateralAmount must be provided together").
>
> **`tradingAccount` is the cross-margin subaccount in both modes.** The isolated account is derived server-side from the signing user, `subaccountId` and `symbol`, so clients never compute its address. Sending the isolated account instead is rejected with `400 BAD_REQUEST`, as is any `tradingAccount` that is not the cross-margin PDA for the signed `subaccountId`. Its address is still what `/deposit` and `/transfer` take (`/withdraw` accepts only cross-margin accounts), and `GET /tradingAccount` reports it with `isolated: true` and its `market`.
>
> **One margin type per market.** A market is either cross-margin or isolated-margin for a given `subaccountId`, never both at once: while an isolated-margin account is live on that market its cross-margin parent reserves the market's position slot for it, and a cross-margin order there is rejected on-chain with `ConflictingMarginType`. The API catches this before publishing and returns `400 CONFLICTING_MARGIN_TYPE` naming the blocking account (e.g. `"Cannot place a cross-margin BTC order: an isolated-margin BTC position or order is live on subaccountId 0 (isolated account …). Close or cancel it, or place this order with isIsolated: true"`). **A resting isolated order is enough** — the isolated position does not have to be open, and an unfilled order left on the book keeps the market locked to isolated margin long after the position is flat. The reservation is released the moment the isolated account is fully unwound: position flat **and** no resting orders. A `POST /order/batch` that cancels on the blocking isolated account (an isolated cancel for the same `subaccountId` and symbol) alongside the cross place exempts that place from the pre-check — the cancel releases the reservation before the place runs — so that is the way to flip a market from isolated to cross in one request. An isolated cancel for a different `subaccountId` does not lift the check. The mirror case is enforced on-chain only: an isolated order on a market where the cross account holds a position or resting orders is rejected with `ConflictingMarginType` on-chain, not by the API.
>
> **Lazy account creation.** The isolated account does not need to be created up front: when the isolated account for the signing user's `(subaccountId, symbol)` does not exist yet, the API creates it (a server-paid `initialize_isolated_margin` transaction, confirmed before the order is accepted) and then processes the order normally — the request shape is unchanged. The first order per `(user, subaccount, market)` therefore takes one extra transaction confirmation; subsequent orders are unaffected. The cross-margin parent at `subaccountId` must already exist — it funds `collateralAmount` — otherwise the request is rejected with `400 BAD_REQUEST`. If the order itself is later rejected the created account simply remains empty and is reused by the next order. `POST /tradingAccount?is_isolated=true` remains available to pre-create the account explicitly.
>
> Market orders are signaled by `orderType: "market"` and use `price = 0` to sweep the book at any price. A non-zero `price` acts as a slippage cap (bid: max paid; ask: min received). Any unfilled remainder is cancelled. The on-chain `PlaceOrderParams` carries an explicit `OrderType` field, and the matcher special-cases `pricePerLot = 0` as "match anything". Order history surfaces these as `orderType: "market"`; once fills exist `price` is the weighted average execution price, while an unfilled entry shows `"0"` (sweep) or the formatted cap.
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
| `{"Seconds": N}` | - | Order expires N seconds after it lands on-chain (N must be > 0). It stops matching at expiry and is removed from the book within a few seconds, which sends a `Cancel` event with reason `"Expired"` |

**Response:**
```json
{
  "symbol": "SOL",
  "clientOrderId": "12345"
}
```

**Example:**
```bash
curl -X POST https://api.gum.ag/perps/order/create \
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
| `isIsolated` | boolean | No | Target the isolated-margin account for `(subaccountId, symbol)` instead of the cross-margin subaccount. Defaults to `false`. `tradingAccount` stays the cross-margin subaccount either way; if no such isolated account exists the request is rejected with `400 BAD_REQUEST` |
| `symbol` | string | Yes | Market symbol |
| `orderId` | string | No | Order ID to cancel |
| `clientOrderId` | string \| number | No | Client order ID to cancel. Decimal string preferred; a number is still accepted |
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
curl -X POST https://api.gum.ag/perps/order/cancel \
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
| `isIsolated` | boolean | No | Target the isolated-margin account for `(subaccountId, symbol)` instead of the cross-margin subaccount. Defaults to `false`. `tradingAccount` stays the cross-margin subaccount either way; if no such isolated account exists the request is rejected with `400 BAD_REQUEST` |
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
curl -X POST https://api.gum.ag/perps/order/cancel-all \
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

### Set Leverage

Set the per-market leverage preference for the signing user. The preference tightens the initial margin ratio to `max(ceil(10000 / leverage), the ratio the position's band charges)`; maintenance margin is unaffected. It is evaluated against the account as it stands, so a live `(user, subaccount, market)` can be re-levered: accepted when current exposure (position + open orders) fits the cap the new leverage allows and equity covers the initial margin recomputed under it, otherwise rejected on-chain with `RiskTierLeverageCapExceeded` or `InsufficientMargin`. Nothing is auto-cancelled or auto-reduced.

An account with no preference for a market is admitted as the first `riskTiers` band's `maxLeverage` — the terms it is charged anyway — so its exposure is capped at that band's `notionalCap` until it sets one. Trading past the first band on a tiered market therefore requires an explicit leverage the higher band admits.

Because setting leverage creates a per-market `LeverageConfig` account on first use (which requires a rent-paying signer), this uses the same **build-then-submit** transaction flow as deposit/withdraw/transfer (see [Signing Deposit, Withdraw, and Transfer Transactions](#signing-deposit-withdraw-and-transfer-transactions)) rather than the signed-action order flow. The server is the fee and rent payer; the user co-signs to authorize the change.

**Endpoints:** `GET /leverage` (build), `POST /leverage` (submit). To read the preferences currently set, use [Get Trading Account Leverage](#get-trading-account-leverage).

> **Lazy account creation (`is_isolated=true`).** The isolated account does not need to exist up front: `GET /leverage` builds against its derived address, and `POST /leverage` creates it (a server-paid `initialize_isolated_margin` transaction, confirmed before the leverage change is submitted) when it is missing — so leverage can be chosen before the market's first isolated position, which is the point of setting it. The first such submit per `(user, subaccount, market)` therefore takes one extra transaction confirmation. The cross-margin parent at `subaccountId` must already exist. Creation runs only after the user's signature on the submitted transaction verifies.

#### `GET /leverage` — build the unsigned transaction

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |

**Query Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `symbol` | string | Yes | Market symbol |
| `leverage` | number | Yes | Integer leverage, `1..=maxLeverage`. `/markets` reports that bound for the market default table; a whitelisted account is bounded by its own table |
| `is_isolated` | boolean | No | Set the isolated-margin preference instead of cross-margin. Defaults to `false` |
| `subaccount_id` | number | No | Subaccount index. Defaults to `0` |

**Response:**
```json
{
  "serializedTransaction": "BASE64_UNSIGNED_TRANSACTION"
}
```

Decode `serializedTransaction`, sign the message bytes at the signature slot whose account key equals `X-PUBKEY`, re-serialize, and submit to `POST /leverage`.

#### `POST /leverage` — submit the signed transaction

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body:**
```json
{
  "serializedTransaction": "BASE64_USER_SIGNED_TRANSACTION"
}
```

**Response:**
```json
{
  "signature": "TRANSACTION_SIGNATURE"
}
```

**Example:**
```bash
# 1. Build
curl "https://api.gum.ag/perps/leverage?symbol=SOL&leverage=5" \
  -H "X-PUBKEY: YOUR_PUBKEY"

# 2. Sign the returned transaction locally, then submit
curl -X POST https://api.gum.ag/perps/leverage \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{ "serializedTransaction": "BASE64_USER_SIGNED_TRANSACTION" }'
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
      "clientOrderId": "1",
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    },
    {
      "method": "cancel",
      "symbol": "SOL",
      "clientOrderId": "1",
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
curl -X POST https://api.gum.ag/perps/order/batch \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{
    "actions": [
      {"method":"place","symbol":"SOL","side":"Bid","orderType":"postOnly","price":99.0,"quantity":5.0,"tradingAccount":"YOUR_TRADING_ACCOUNT","recentBlockhash":"BLOCKHASH","signature":"SIGNATURE"}
    ]
  }'
```

---

## Conditional Orders (TP/SL)

Take-profit / stop-loss settings on an open position. Nothing rests on the book
until the watched price (`triggerPriceType`) crosses the trigger; a keeper then
submits a reduce-only order.

Settings live in a `ConditionalOrders` account per `(trading account, market)` with
**8 slots**, shared between take profits and stop losses. That is the hard cap per
account and symbol — cross and each isolated account get their own 8 — and a create
past it is `ConditionalOrdersFull` (80). An update reuses its setting's slot. The API
creates the account on the first create, server-paid.

### Create Conditional Order

**Endpoint:** `POST /conditionalOrder`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |
| `Content-Type` | Yes | `application/json` |

**Request Body (take profit, percent of position):**
```json
{
  "subaccountId": 0,
  "symbol": "SOL",
  "kind": "TakeProfit",
  "trigger": { "type": "price", "value": 120.0 },
  "triggerPriceType": "MarkPrice",
  "closeAmount": { "type": "bps", "value": 5000 },
  "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
  "recentBlockhash": "BLOCKHASH",
  "signature": "SIGNATURE"
}
```

**Request Body (stop loss, fixed size with a limit price):**
```json
{
  "symbol": "SOL",
  "kind": "StopLoss",
  "trigger": { "type": "pnl", "value": -250.0 },
  "triggerPriceType": "MarkPrice",
  "closeAmount": { "type": "quantity", "value": 5.0 },
  "limitPrice": 89.5,
  "clientOrderId": "77",
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
| `isIsolated` | boolean | No | Attach to the isolated-margin account instead of the cross position. Defaults to `false` |
| `kind` | string | Yes | `"TakeProfit"` or `"StopLoss"` |
| `trigger` | object | Yes | Where the trigger sits, as `{"type": ..., "value": ...}`. See below |
| `triggerPriceType` | string | Yes | Price the trigger watches: `"MarkPrice"` or `"LastPrice"` |
| `closeAmount` | object | Yes | Close size, as `{"type": ..., "value": ...}`. See below |
| `limitPrice` | number | No | Limit price for the submitted order. Omitted = market-after-trigger |
| `clientOrderId` | string \| number | No | Client id, scoped to `(tradingAccount, subaccountId, isIsolated, symbol)`. Send as a decimal string |
| `tradingAccount` | string | Yes | The user's cross-margin subaccount, for isolated settings too |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

**`trigger`** is a tagged union. Whichever form is used, it must resolve to the
profitable side of entry for a take profit and the losing side for a stop loss:

| `type` | `value` | Meaning |
|--------|---------|---------|
| `"price"` | number | Price the trigger fires at, on the `triggerPriceType` source |
| `"pnl"` | number | How far the position is up or down, in quote, at the trigger |
| `"roiPercent"` | number | Return on the position's initial margin, as a percent (`10` = 10%, `12.5` = 12.5%) |

`pnl` and `roiPercent` are **signed by kind**: positive for a take profit, which
names a gain, negative for a stop loss, which names a loss. A sign disagreeing
with `kind` is a `400` rather than a silently flipped trigger, and `0` is
rejected — it is no distance off entry. The magnitude is what reaches the chain;
`kind` carries the direction. `roiPercent` is converted to basis points on the
way in (`10` becomes `1000`), and a percent below one basis point is a `400`.

`pnl` and `roiPercent` are resolved against the live position **on-chain**, so the
price they derive uses the position as it stands when the setting is armed, not a
snapshot. Only the derived price is stored, and it is the only thing compared
against the mark — the raw input is reported once on the `ConditionalOrderUpsert`
event and indexed there, not held in the setting. Rounding runs
toward increasing position PnL — a long ceils to the tick, a short floors — so a
take profit lands no closer to entry than asked and a stop loss no further from it.
`roiPercent` measures against the position's effective initial margin ratio
(`effectiveImrBps` on the positions endpoint), with the tier taken at the entry
notional, so a `10%` ROI at a 10% ratio moves the trigger 1% off entry.

**`triggerPriceType`** selects what the keeper compares the trigger against:
`"MarkPrice"` the oracle mark, `"LastPrice"` the market's last on-chain trade.
A `"LastPrice"` setting only fires on a trade made at or after the time it was
created or last updated, so a trade from before then never fires it. Margin,
liquidation, OI notional, and P0-2 price protection always use the mark,
whatever a setting watches.

**`closeAmount`** is a tagged union — one `type`, one `value` — so a body can carry
exactly one size:

| `type` | `value` | Meaning |
|--------|---------|---------|
| `"quantity"` | number | Base units, e.g. `5.0` SOL, as an order's `quantity` |
| `"bps"` | integer | Basis points of the position at trigger time, `0 < value <= 10000` (`5000` = 50%) |
| `"notional"` | number | Quote notional, e.g. `300.0` USDC, resolved into base units at `triggerPrice` |

`bps` is an integer so a close size can never be silently rounded on the way in;
a fractional `value` is a `400`.

**Response:**
```json
{
  "success": true
}
```

Accepted and published — not armed. Confirm on the WebSocket `ConditionalOrderUpsert`
event; an on-chain rejection arrives as `Error`.

**Rejections:**

| Condition | Error (custom code) |
|-----------|---------------------|
| No open position on the market | `NoPositionForConditionalOrder` (75) |
| `limitPrice` past the trigger — above it for a long's close, below it for a short's | `ConditionalLimitPriceUnfillable` (85) |
| `limitPrice` too large to price the position's close | `OrderSizeAboveLimit` (13) |
| Trigger on the wrong side of entry — it must be above for a long's TP or a short's SL, below for the mirrors. A `pnl` / `roiPercent` too small to clear a tick lands back on entry and is rejected here | `InvalidTriggerPrice` (76) |
| `closeAmount` missing, or an unknown `type` | `400 BAD_REQUEST` |
| All 8 slots armed | `ConditionalOrdersFull` (80) |
| Same-kind sizes would sum past the position | `ConditionalOrderAggregateExceedsPosition` (82) |
| A live `clientOrderId` given a differing create | `ConditionalOrderClientIdConflict` (83) |
| Account under liquidation | `AccountUnderLiquidation` |

**Notes:**

- A create never replaces. Each takes a slot and mints a fresh `internalId`; to change
  a setting, `POST /conditionalOrder/update` re-arms it in place under the same id.
- Sizes aggregate per kind, independently: two 50% take profits are fine, a third is
  not, and a full-size stop loss alongside them is still fine.
- `bps` is measured at trigger time, so ladders compound — 5000 then 5000 closes
  75% of the original position. `quantity` is clipped to the position. A bps size
  that re-measures to nothing cancels the setting (`reason: 1`).
- `notional` divides by `triggerPrice`, not by the mark at trigger time, so the size
  it aggregates against at create is the one it spends when it fires. It floors to
  whole base lots, and a notional past what the position is worth at the trigger is
  rejected like an oversized `quantity`.
- `limitPrice` must be reachable by the crossing that fires the setting: at or below
  the trigger when the close sells (a long), at or above it when the close buys (a
  short). Past it the IOC could only expire, so arming is refused. Its distance from
  the market is not checked at create: if it sits outside the market's price band
  when the setting fires, the trigger cancels it outright (`reason: 3`) rather than
  spending the retry budget on a price that cannot change.
- A fired setting **outranks the account's own resting reduce-only orders** on that
  side. Any that no longer fit the position once the trigger's order is placed are
  removed, including when the trigger's IOC goes on to fill nothing. Their owner did
  not cancel them, so they end at status `"expired"`, not `"cancelled"`, and their
  cancel transaction carries `reason: "TpslTriggerPriority"` with the `tpslId` and
  `tpslKind` of the setting that displaced them. Resting orders that are *not*
  reduce-only are left alone. Note the same-kind aggregate check at create counts only settings, not
  resting reduce-only orders.
- `clientOrderId` is unique per `(tradingAccount, subaccountId, isIsolated, symbol)`
  among **armed** settings and never collides with an order's. Every lookup and cancel
  therefore takes the symbol, and the aggregated listing may return several sharing one
  id: the same id can be live on two symbols at once, one of them isolated — a market is
  never both margin types at once, so cross and isolated share an id only across symbols.
  A triggered order carries none — correlate it by the trigger event's `orderId`.
- An identical repeat of a live `clientOrderId` returns `success` and creates nothing;
  a differing one is `ConditionalOrderClientIdConflict`. The id frees once the setting
  is cancelled or fires.
- While the account is under liquidation, settings cannot be armed, updated or triggered,
  but they stay armed. Once it recovers with the original position still open on the same
  side, they resume with their original trigger prices and close at most what is left of
  the position. A setting whose position the liquidation closed is cleared with `reason: 1`
  when a keeper next touches it, and never fires on a position opened later.
- Pending fills on the event heap are consumed first, so an uncranked maker fill still
  counts as a position.

**Example:**
```bash
curl -X POST https://api.gum.ag/perps/conditionalOrder \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"SOL","kind":"TakeProfit",
       "trigger":{"type":"price","value":120.0},
       "triggerPriceType":"MarkPrice",
       "closeAmount":{"type":"bps","value":5000},
       "tradingAccount":"...","recentBlockhash":"...","signature":"..."}'
```

### Update Conditional Order

**Endpoint:** `POST /conditionalOrder/update`

Re-arms one live setting in place. It keeps its slot, its `internalId`, and its
`clientOrderId`; everything else is replaced by what the request carries, so an
update sends the terms in full rather than a patch — an omitted `limitPrice`
means market-after-trigger, not "leave it as it was".

**Request Body:**
```json
{
  "symbol": "SOL",
  "kind": "TakeProfit",
  "internalId": 3,
  "trigger": { "type": "price", "value": 130.0 },
  "triggerPriceType": "MarkPrice",
  "closeAmount": { "type": "bps", "value": 5000 },
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
| `isIsolated` | boolean | No | Target the isolated-margin account. Defaults to `false` |
| `kind` | string | Yes | `"TakeProfit"` or `"StopLoss"` — the kind the setting already has, part of what addresses it |
| `internalId` | number | No | The setting to re-arm. Exactly one of this and `clientOrderId` |
| `clientOrderId` | string \| number | No | The setting armed under this client id on `symbol` |
| `trigger` | object | Yes | New trigger, same tagged union as create |
| `triggerPriceType` | string | Yes | `"MarkPrice"` or `"LastPrice"` |
| `closeAmount` | object | Yes | New close size, same tagged union as create |
| `limitPrice` | number | No | New limit price. Omitted = market-after-trigger |
| `tradingAccount` | string | Yes | The user's cross-margin subaccount |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

**Response:**
```json
{
  "success": true
}
```

Accepted and published, as a create is. Confirm on the WebSocket
`ConditionalOrderUpsert` event, which reports the new terms with
`isUpdate: true`.

**Rejections:** the new terms are validated exactly as a create's, against the
position as it stands when the update is applied — wrong side of entry is
`InvalidTriggerPrice` (76), an oversized `closeAmount` is
`ConditionalOrderAggregateExceedsPosition` (82), and a `limitPrice` past the
trigger is `ConditionalLimitPriceUnfillable` (85). The setting's own old size
leaves the aggregate first, so re-arming a full-size setting at full size fits.

| Condition | Error (custom code) |
|-----------|---------------------|
| No armed setting of that `kind` under that id | `ConditionalOrderNotFound` (81) |
| Neither id given, or both | `400 BAD_REQUEST` |
| No conditional orders on the symbol | `404 NOT_FOUND` |
| No open position on the market | `NoPositionForConditionalOrder` (75) |
| Account under liquidation | `AccountUnderLiquidation` |

**Notes:**

- The kind cannot change: an id that names a setting of the other kind is
  `ConditionalOrderNotFound`, not a conversion.
- A setting that has already fired or been cancelled is gone, so an update
  naming it finds nothing. There is no update-or-create.
- The trigger's retry budget resets: an update is new terms, so the attempts
  charged against the old ones do not carry over.

**Example:**
```bash
curl -X POST https://api.gum.ag/perps/conditionalOrder/update \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"SOL","kind":"TakeProfit","internalId":3,
       "trigger":{"type":"price","value":130.0},
       "triggerPriceType":"MarkPrice",
       "closeAmount":{"type":"bps","value":5000},
       "tradingAccount":"...","recentBlockhash":"...","signature":"..."}'
```

### Cancel Conditional Order

**Endpoint:** `POST /conditionalOrder/cancel`

**Request Body (one setting):**
```json
{
  "symbol": "SOL",
  "internalId": 3,
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
| `isIsolated` | boolean | No | Target the isolated-margin account. Defaults to `false` |
| `internalId` | number | No | Cancel this setting |
| `clientOrderId` | string \| number | No | Cancel the setting armed under this client id on `symbol` |
| `tradingAccount` | string | Yes | The user's cross-margin subaccount |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

**Response:**
```json
{
  "success": true
}
```

**Notes:**

- `internalId` and `clientOrderId` are mutually exclusive — both is `400 BAD_REQUEST`.
  Neither cancels every armed setting on the symbol.
- A target matching nothing clears nothing and still succeeds, so a cancel racing a
  trigger is a no-op rather than an error.
- Nothing ever armed on the symbol is `404 NOT_FOUND`.

### Get Conditional Orders

**Endpoint:** `GET /tradingAccount/{trading_account}/conditionalOrders`

**Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-PUBKEY` | Yes | User's public key |

**Response:**
```json
[
  {
    "internalId": 3,
    "symbol": "SOL",
    "kind": "TakeProfit",
    "side": "Ask",
    "triggerPrice": "120.0",
    "triggerPriceType": "MarkPrice",
    "closeAmount": { "type": "bps", "value": 5000 },
    "isolated": false
  },
  {
    "internalId": 4,
    "symbol": "SOL",
    "kind": "StopLoss",
    "side": "Ask",
    "triggerPrice": "90.0",
    "triggerPriceType": "MarkPrice",
    "closeAmount": { "type": "quantity", "value": "5.0" },
    "limitPrice": "89.5",
    "clientOrderId": "77",
    "isolated": false
  },
  {
    "internalId": 5,
    "symbol": "SOL",
    "kind": "TakeProfit",
    "side": "Ask",
    "triggerPrice": "150.0",
    "triggerPriceType": "MarkPrice",
    "closeAmount": { "type": "notional", "value": "300" },
    "isolated": false
  }
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `internalId` | number | Addresses the setting for its whole life. What a cancel targets and what the events carry. Program-generated, so a number rather than a string |
| `symbol` | string | Market symbol |
| `kind` | string | `"TakeProfit"` or `"StopLoss"` |
| `side` | string | Side the trigger closes on: `"Ask"` for a long, `"Bid"` for a short |
| `triggerPrice` | string | Price the trigger fires at, derived when the create named a PnL or an ROI |
| `triggerPriceType` | string | `"MarkPrice"` or `"LastPrice"` |
| `closeAmount` | object | Close size, tagged as on create. `value` is a decimal string for `quantity` / `notional` and an integer for `bps` |
| `limitPrice` | string | Limit price of the submitted order. Omitted = market-after-trigger |
| `clientOrderId` | string | The client id the setting was armed under, if any |
| `isolated` | boolean | `true` when the setting belongs to an isolated-margin account |

**Notes:**

- A cross-margin subaccount's listing includes its isolated accounts' settings, each
  flagged with `isolated`.
- Only settings bound to the live position instance are listed. One left behind by a
  close, a flip, a reopen or a liquidation is omitted rather than reported as armed:
  it can no longer fire, and a keeper will clear it (`reason: 1`) when it next touches
  the account, which for an uncrossed trigger may be a while.

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount/TRADING_ACCOUNT_PUBKEY/conditionalOrders \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

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
- **Cross-margin (default):** the server automatically selects the next available subaccount ID (0-255). Each user can have at most **5** cross-margin subaccounts; further requests fail with `400 BAD_REQUEST`.
- **Isolated-margin (`is_isolated=true`):** the cross-margin parent at `subaccount_id` must already exist — create it first with a plain `POST /tradingAccount`. The isolated account is keyed by `(user, subaccount_id, market)` and is created empty; fund it directly via [`/deposit`](#deposit) using the returned `tradingAccount` address.
- Once created and funded, place orders against it via [Place Order](#place-order): pass the **cross-margin parent** as `tradingAccount` with the matching `subaccountId`, `isIsolated: true` and a `collateralAmount` (use `0` to draw only on the isolated account's own balance) to route to `place_order_isolated`. The address returned here is not what orders are addressed to — it is what `/deposit` and `/transfer` take (withdrawing requires transferring to the cross-margin parent first), and it is also reported by [`GET /tradingAccount`](#list-trading-accounts) with `isolated: true`.
- **Pre-creating an isolated account is optional.** An isolated [Place Order](#place-order) (`isIsolated: true` + `collateralAmount`) creates a missing isolated account automatically before the order is accepted; this endpoint remains for explicitly creating one (e.g. to `/deposit` into it before trading).

**Example (cross-margin):**
```bash
curl -X POST https://api.gum.ag/perps/tradingAccount \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

**Example (isolated-margin on ETH, parent subaccount 0):**
```bash
curl -X POST "https://api.gum.ag/perps/tradingAccount?is_isolated=true&symbol=ETH&subaccount_id=0" \
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
curl -X POST https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/name \
  -H "X-PUBKEY: YOUR_PUBKEY" \
  -H "Content-Type: application/json" \
  -d '{"name":"Main","blockhash":"RECENT_BLOCKHASH","signature":"USER_SIGNATURE"}'
```

---

### List Trading Accounts

List all trading accounts for a user: every cross-margin subaccount plus every isolated-margin account under them, sorted by `subaccountId`. Use this to look up an isolated account's address for `/deposit` and `/transfer` — orders and subscriptions use the cross-margin subaccount instead, and `/withdraw` accepts only cross-margin accounts.

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
      "isolated": false,
      "market": null,
      "data": {
        "freeBalance": "1000.00",
        "availableToTrade": "1200.50",
        "tokenBalance": 1000000000,
        "accountValue": "1250.75",
        "totalMaintenanceMargin": "50.25",
        "totalLiquidationRequirement": "55.25",
        "crossAccountLeverage": "2.5"
      }
    },
    {
      "pubkey": "ISOLATED_TRADING_ACCOUNT_PUBKEY",
      "subaccountId": 0,
      "name": null,
      "isolated": true,
      "market": "SOL",
      "data": {
        "freeBalance": "50.00",
        "availableToTrade": "0.00",
        "tokenBalance": 50000000,
        "accountValue": "50.00",
        "totalMaintenanceMargin": "5.00",
        "totalLiquidationRequirement": "5.50",
        "crossAccountLeverage": "1.0"
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
| `accounts[].isolated` | boolean | True for an isolated-margin account. Isolated accounts are listed alongside the cross-margin subaccount they settle into and **share its `subaccountId`**, so this and `market` are what tell the entries apart |
| `accounts[].market` | string \| null | Symbol an isolated account is bound to; `null` for cross-margin subaccounts |
| `accounts[].data` | object | Account data. The endpoint returns `SERVICE_DEGRADED` instead of partial account entries when required account snapshots are missing |
| `accounts[].data.freeBalance` | string | Withdrawable balance in quote units: what `/withdraw`, `/transfer` and new isolated orders (funded from the cross-margin parent) can use. Collateral plus unrealized PnL minus initial margin: losses count in full, profit only up to the vault's unrealized-profit transfer ratio (0% by default, so profit is excluded) |
| `accounts[].data.availableToTrade` | string | Margin left for new orders on a cross-margin account, in quote units: account value (unrealized profit counted in full) minus the initial margin of positions and open orders. A margin amount, not order notional; fees, slippage and limit prices away from the mark also consume it. Always `"0.00"` for isolated accounts, whose orders are funded from the parent's `freeBalance` |
| `accounts[].data.tokenBalance` | number | Single token balance in smallest units |
| `accounts[].data.accountValue` | string \| null | USD-denominated equity: collateral + unsettled PnL/funding across active positions; `null` when active account risk cannot be priced from the current market cache |
| `accounts[].data.totalMaintenanceMargin` | string \| null | Maintenance margin requirement summed across active positions, in USD (open orders reserve initial margin only); `null` when active account risk cannot be priced from the current market cache |
| `accounts[].data.totalLiquidationRequirement` | string \| null | Fee-aware liquidation requirement in USD: maintenance margin plus close-fee and liquidation buffers; `null` when active account risk cannot be priced from the current market cache |
| `accounts[].data.crossAccountLeverage` | string \| null | Sum of open-position notional / `accountValue`. `0` when `accountValue <= 0` or there are no open positions; `null` when active account risk cannot be priced from the current market cache |

**Notes:**
- Use `/tradingAccount/{trading_account}/positions` for position data
- Use `/tradingAccount/{trading_account}/orders` for open orders

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount \
  -H "X-PUBKEY: YOUR_PUBKEY"
```

---

### Get Trading Account Orders

Get all open orders for a trading account across all markets.

> **Family aggregation.** Querying a cross-margin subaccount also returns the orders resting on its isolated-margin accounts (same user and `subaccountId`), matching `GET /positions`. Querying an isolated account directly returns only its own orders. If an isolated account's on-chain snapshot is temporarily unavailable, its orders are omitted from the aggregated listing rather than failing the request; the queried account's own snapshot is still required (`SERVICE_DEGRADED` when missing).

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
      "clientOrderId": "12345",
      "reduceOnly": false,
      "isolated": false,
      "collateral": null
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
| `status` | string | Order status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"`, `"expired"` |
| `clientOrderId` | string \| null | Client-provided order ID as a decimal string (omitted if not set) |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |
| `isolated` | boolean | True when the order rests on an isolated-margin account. A cross-margin subaccount's listing includes its isolated accounts' orders, and cancelling one requires `isIsolated: true`, so this is what makes an aggregated order cancellable |
| `collateral` | string \| null | For an isolated order, the USDC collateral the chain currently reserves against it (USD) — the amount fenced for this order alone, not the isolated account's whole balance. `null` for cross-margin orders, which draw on shared account-level collateral |

> **`collateral` tracks `remainingSize`, not the amount funded at placement.** The reservation is released proportionally into the isolated account's own collateral as the order fills, so a half-filled order reports roughly half of what it was placed with. An isolated order always reports a number, so `"0"` (reservation fully drained) stays distinguishable from `null` (cross-margin, not applicable). A reduce-only isolated order reserves nothing at placement, so it reports `"0"` for its whole life.

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/orders
```

---

### Get Trading Account Positions

Get all perpetual positions for a trading account. For a cross-margin account
this includes the positions held by its isolated-margin accounts (same user and
subaccount), each marked with `isolated: true`. Querying an isolated account
directly returns only its own position.

**Endpoint:** `GET /tradingAccount/{trading_account}/positions`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key (cross or isolated) |

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
    "pendingFundingReceivable": "1.25",
    "totalFunding": "6.5",
    "selectedLeverage": 5,
    "effectiveImrBps": 2000,
    "maintenanceMarginRatioBps": 800,
    "maintenanceAmount": "0",
    "tierIndex": 0,
    "tierTableIdx": 0,
    "isolated": false,
    "collateral": null,
    "adlQuantile": 2
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
| `liquidationPrice` | string \| null | Estimated liquidation price (`null` when no estimate is available). For a cross-margin position it accounts for the shared collateral across all of the account's positions; for an isolated position it is computed against the isolated account's own collateral only |
| `fundingAccrued` | string | Unsettled funding accrued on this position (USDC). Positive = funding received, negative = funding paid |
| `pendingFundingPayable` | string | Pending funding this position still owes (USDC, >= 0). Mirrors the negative side of `fundingAccrued` |
| `pendingFundingReceivable` | string | Pending funding this position is still owed (USDC, >= 0). A pending receivable only — not claimable, transferable, or withdrawable until actually settled |
| `totalFunding` | string | Total funding this position has accrued since it was opened (USDC): payments already settled to collateral plus the pending amount. Positive = funding received, negative = funding paid. Starts from zero when the position opens from flat or flips side; adding to or partially closing the position keeps the running total. A position already open when the program began tracking it counts from the program upgrade onward |
| `selectedLeverage` | number | Leverage currently applied to the position, updated in place by `POST /leverage` (0 = none selected) |
| `effectiveImrBps` | number | Initial margin ratio applied to this position in bps; the market ratio when no leverage is selected |
| `maintenanceMarginRatioBps` | number | Maintenance margin ratio applied to this position in bps |
| `maintenanceAmount` | string | Tier deduction keeping maintenance continuous with the band below (USDC). Maintenance margin is `notional × maintenanceMarginRatioBps / 10000 - maintenanceAmount`, floored at 0 |
| `tierIndex` | number \| null | Band the position's notional resolves to, indexed into the tier table. `null` for legacy positions and untiered markets, which margin on the market's flat ratios |
| `tierTableIdx` | number | Tier table this position margins through. `0` is the market default returned by `/markets`; a whitelisted account can be pinned to another table |
| `isolated` | boolean | `true` when the position is held in an isolated-margin account (backed by that account's own collateral) rather than the cross-margin account it is listed under |
| `collateral` | string \| null | For an isolated position, the USDC collateral held by its isolated account — the equity backing the position (deposits ± realized PnL and fees, whether or not yet settled, excluding the open lots' unrealized PnL and funding). Can be negative while a bankrupt position is being liquidated. `null` for cross-margin positions, which share the account-level collateral reported by the trading-account endpoints |
| `adlQuantile` | number \| null | ADL indicator: where the position stands in the market's auto-deleveraging queue for its side, which ranks profitable positions by profit percentage × effective leverage. `4` = top 10% (the head of the queue is always `4`), `3` = top 25%, `2` = top 50%, `1` = the rest, `0` = not in the queue (not profitable, account under liquidation, or ADL disabled on the market; see `isAdlEnabled` on `/markets`). Re-ranked about every 5 seconds; `null` while the indicator is unavailable. Current standing only, not a prediction that the position will or will not be deleveraged |

**Notes:**
- Unrealized PnL is calculated using `markPrice`. In phase one, `markPrice = indexPrice`.
- Liquidation price accounts for cross-margin (shared collateral across all positions). Isolated positions are margined by their own account's collateral; collateral reserved for resting isolated orders is fenced separately and does not affect the position's liquidation price.
- Margin is calculated as: notional_value × initial_margin_ratio
- `totalFunding` need not equal the sum of `GET /tradingAccount/{trading_account}/funding` entries since the open: payments settle to collateral after the funding accrues, so funding a previous position in the market accrued before it closed can be paid after the reopen

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/positions
```

---

### Get Trading Account Leverage

Get the per-market leverage preferences set via [`POST /leverage`](#set-leverage), including markets with no open position. For a cross-margin account this includes the preferences of its isolated-margin accounts (same user and subaccount), each marked with `isolated: true`. Querying an isolated account directly returns only its own preference.

**Endpoint:** `GET /tradingAccount/{trading_account}/leverage`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key (cross or isolated) |

**Response:**
```json
[
  {
    "symbol": "SOL",
    "isolated": false,
    "leverage": 5
  },
  {
    "symbol": "ETH",
    "isolated": true,
    "leverage": 20
  }
]
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol. Falls back to the market public key if the symbol cannot be resolved |
| `isolated` | boolean | `true` for the preference of the market's isolated-margin account, `false` for the cross-margin account |
| `leverage` | number | Leverage last set for the market |

**Notes:**
- Markets with no preference set are omitted; they are admitted as described in [Set Leverage](#set-leverage).
- Served from indexed transactions, so a change made by `POST /leverage` appears once it is indexed. The WebSocket `LeverageUpdate` event delivers the change as it lands.

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/leverage
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
  "availableToTrade": "1200.50",
  "tokenBalance": 1000000000
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `freeBalance` | string | Withdrawable balance in quote units: what `/withdraw`, `/transfer` and new isolated orders can use. Unrealized profit counts only up to the vault's transfer ratio (none by default). The endpoint returns `SERVICE_DEGRADED` when the required account snapshot is missing |
| `availableToTrade` | string | Margin left for new cross-margin orders, in quote units, with unrealized profit counted in full. `"0.00"` for isolated accounts. See [List Trading Accounts](#list-trading-accounts) |
| `tokenBalance` | number | Single token balance in smallest units |

**Example:**
```bash
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/balances
```

---

### Get Trading Account Trades

Get trade history for a trading account.

> **Family aggregation.** Querying a cross-margin subaccount also returns the rows of its isolated-margin accounts (same user and `subaccountId`). Querying an isolated account directly returns only its own rows.

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
| `endTime` | number | No | Inclusive end timestamp (Unix seconds), applied only when `startTime` is provided. When omitted there is no upper time bound; without `startTime`, the endpoint returns the latest 500 trades and ignores `endTime` |

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
    "clientOrderId": "123456",
    "fee": "0.050250",
    "realizedPnl": "5.25",
    "isolated": false
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
| `clientOrderId` | string \| null | Client-provided order ID for this trading account's side of the fill, as a decimal string. Null if the order was submitted without a client order ID |
| `fee` | string | Fee charged for this trade in USDC (negative for maker rebates) |
| `realizedPnl` | string \| null | Realized PnL excluding fees. Null if the trade opened or increased a position. Populated for both maker and taker fills that reduce an existing position; a fill that flips the position counts only the lots that closed it |
| `isolated` | boolean | `true` when this trading account's side of the fill was executed by an isolated-margin account. A cross-margin subaccount's aggregated listing uses this to tell its isolated accounts' fills apart |

**Example:**
```bash
# Get last 500 trades
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/trades

# Get trades for specific market
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/trades?symbol=SOL"

# Get trades in time range
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/trades?startTime=1702656000&endTime=1702742400"
```

---

### Get Position History

Get the closed positions of a trading account, newest close first. Open positions are listed by `GET /tradingAccount/{trading_account}/positions`.

> **Family aggregation.** Querying a cross-margin subaccount also returns the rows of its isolated-margin accounts (same user and `subaccountId`). Querying an isolated account directly returns only its own rows.

**Endpoint:** `GET /tradingAccount/{trading_account}/positionHistory`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | No | Filter by market symbol (e.g., "SOL"). An unknown symbol returns no entries |
| `limit` | number | No | Maximum entries to return (default: 100, max: 500) |
| `offset` | number | No | Offset for pagination (default: 0) |
| `startTime` | number | No | Unix timestamp - filter positions closed at or after this time |
| `endTime` | number | No | Unix timestamp - filter positions closed at or before this time |

**Response:**
```json
{
  "entries": [
    {
      "symbol": "SOL",
      "side": "Bid",
      "isolated": false,
      "entryPrice": "150.2",
      "exitPrice": "155.8",
      "closedSize": "2.5",
      "realizedPnl": "14",
      "fees": "0.765",
      "funding": "-0.12",
      "netPnl": "13.115",
      "closeReason": "trade",
      "openedAt": 1761091200,
      "closedAt": 1761120000
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `entries` | array | Closed positions, sorted by `closedAt` descending |
| `entries[].symbol` | string | Market symbol |
| `entries[].side` | string | Side of the closed position: `"Bid"` for long or `"Ask"` for short |
| `entries[].isolated` | boolean | `true` when the position was held in an isolated-margin account rather than the cross-margin account it is listed under |
| `entries[].entryPrice` | string | Average price of every fill that opened or added to the position |
| `entries[].exitPrice` | string | Average price of every fill that reduced or closed the position |
| `entries[].closedSize` | string | Total size closed over the position's life in base units. Adds and partial closes count, so it equals the total size opened and can exceed the largest size held |
| `entries[].realizedPnl` | string | Price PnL in USDC, excluding fees and funding: `closedSize × (exitPrice − entryPrice)` for a long, negated for a short |
| `entries[].fees` | string | Fees in USDC, positive = paid: trading fees on every fill of the position (net of maker rebates, so it can be negative) plus any liquidation fee. A fill that flips the position splits its fee pro rata between the closed and the new position |
| `entries[].funding` | string | Funding accrued over the position's life in USDC, positive = received |
| `entries[].netPnl` | string | `realizedPnl − fees + funding`, in USDC |
| `entries[].closeReason` | string | What closed the position: `"trade"` (an order, including a flip to the opposite side), `"liquidation"` or `"adl"` (auto-deleveraging) |
| `entries[].openedAt` | number | Unix timestamp (seconds) of the first opening fill |
| `entries[].closedAt` | number | Unix timestamp (seconds) of the fill that closed the position |

**Notes:**
- An entry covers one position from opening (from flat, or by flipping side) until it goes flat or flips again. A flip closes one entry and opens the next
- Only positions opened after the program upgrade that tags fills with their position (see the 2026-10-27 changelog entry) are tracked. Positions already open at the upgrade are not listed, even after they close
- A closed position is listed once all of its fills are indexed (its opened and closed sizes match); until then it is omitted rather than shown with partial prices or PnL
- Trading account pubkey is validated (returns 400 if invalid)

**Example:**
```bash
# Get the latest closed positions
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/positionHistory

# Get closed positions for a specific market with pagination
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/positionHistory?symbol=SOL&limit=50&offset=0"

# Get positions closed in a time range
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/positionHistory?startTime=1761091200&endTime=1761177600"
```

---

### Get Order History

Get historical orders (including filled, cancelled, and open orders) for a trading account.
Liquidation attempts that execute zero base size are omitted.

> **Family aggregation.** Querying a cross-margin subaccount also returns the rows of its isolated-margin accounts (same user and `subaccountId`). Querying an isolated account directly returns only its own rows.

**Endpoint:** `GET /tradingAccount/{trading_account}/orderHistory`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | No | Filter by market symbol (e.g., "SOL") |
| `status` | string | No | Filter by history status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"`, `"partially_filled_cancelled"`, `"partially_liquidated"`, `"liquidated"`, `"espired"` |
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
      "clientOrderId": "12345",
      "symbol": "SOL",
      "side": "Bid",
      "price": "100.50",
      "originalSize": "10.5",
      "filledSize": "5.25",
      "realizedPnl": null,
      "status": "partially_filled",
      "orderType": "limit",
      "timeInForce": "gtc",
      "reduceOnly": false,
      "isolated": true,
      "initialCollateral": "500",
      "executedCollateral": "200",
      "originTxId": null,
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
| `orders[].clientOrderId` | string \| null | Client-provided order ID as a decimal string, or `null` if not set |
| `orders[].symbol` | string | Market symbol (e.g., "SOL") |
| `orders[].side` | string | Order side: `"Bid"` or `"Ask"` |
| `orders[].price` | string | Parent order price. Once an order has fill transactions, this is the base-size-weighted average execution price; unfilled orders show the submitted limit price. |
| `orders[].originalSize` | string | Original order size in base units |
| `orders[].filledSize` | string | Filled size in base units |
| `orders[].realizedPnl` | string \| null | Realized PnL in USDC summed from this order's closing fill transactions; `null` when the order has no realized PnL. Fees are exposed separately on fill transactions and are not netted into this value. |
| `orders[].status` | string | History status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"`, `"partially_filled_cancelled"`, `"partially_liquidated"`, `"liquidated"`, `"expired"`. A liquidation order that closed part of the position to step it down a risk tier is `"partially_liquidated"` (display label: **Partially liquidated**). A liquidation order sized to close the whole position is `"liquidated"` (display label: **Liquidated**), including one whose unfilled remainder was cancelled. Both statuses are terminal for that order, not proof that the position or account is fully closed, and later attempts appear separately. Liquidations executed before the program recorded tier steps read `"liquidated"`. The `status` filter uses the same classification. The `transactions` trail retains any remainder cancellation. Any other order cancelled after a partial fill, including an ADL order or the unfilled remainder of an IOC or market order, is `"partially_filled_cancelled"` (display label: **Partially filled, rest cancelled**); `"cancelled"` means nothing filled. `"expired"` means removed by the system rather than by its owner, currently only a reduce-only order displaced by a fired TP/SL |
| `orders[].orderType` | string | Order type: `"limit"`, `"market"`, `"postOnly"`, `"liquidation"`, or `"adl"` |
| `orders[].timeInForce` | string | Time in force: `"gtc"`, `"gtd"`, `"ioc"`, `"fok"`, `"liquidation"`, or `"adl"` |
| `orders[].reduceOnly` | boolean | Whether the order was submitted as reduce-only |
| `orders[].isolated` | boolean | `true` when the order was placed on an isolated-margin account. A cross-margin subaccount's aggregated listing uses this to tell its isolated accounts' orders apart |
| `orders[].initialCollateral` | string \| null | USDC budget the **isolated** order committed from its cross-margin parent at placement, as sent by the client. `null` for cross-margin orders (not applicable) and for orders placed before this field existed — their V1 submit carries no budget, even if reindexed later (unknown); an isolated order always reports a number, so `"0"` is a confirmed zero commitment. |
| `orders[].executedCollateral` | string \| null | Signed USDC net of the order's execution: the consumed share of the initial budget (drawn into the position — positive) minus the equity its executed reductions settled back to the cross parent (a close order reads negative, e.g. `"-200"`; that equity lands on the parent's shadow position and the settler realizes it to cash afterwards, reported by `Settle`). For a cancelled order the consumed share is exact — the on-chain draw minus what the release log returned to the parent. Otherwise it is the filled share (`initialCollateral × filledSize / originalSize`) capped at the actual drawn amount, so it never exceeds what the parent really paid, and an order that drew nothing (pure reduction) nets pure settle-back even if a budget was attached. `null` for cross-margin orders and for orders placed before the field existed; an isolated order always reports a number, so `"0"` stays distinguishable from unknown. |
| `orders[].originTxId` | string \| null | Source-chain transaction of an order placed through the Gum app's crosschain flow; `null` for orders placed through this API |
| `orders[].createdAt` | number | Unix timestamp when order was created |
| `orders[].updatedAt` | number | Unix timestamp of last update |
| `orders[].transactions` | array | List of transactions related to this order |
| `orders[].transactions[].signature` | string | Transaction signature |
| `orders[].transactions[].type` | string | Transaction type: `"submit"`, `"fill"`, `"cancel"` |
| `orders[].transactions[].timestamp` | number | Unix timestamp |
| `orders[].transactions[].price` | string? | Fill price (only for `"fill"` transactions) |
| `orders[].transactions[].size` | string? | Fill size in base units (only for `"fill"` transactions) |
| `orders[].transactions[].fee` | string? | Normal maker/taker trading fee in USDC (only for `"fill"` transactions; excludes liquidation fee) |
| `orders[].transactions[].realizedPnl` | string? | Realized PnL in USDC for this closing fill transaction, counting only the lots that reduced the position when the fill flips it. Omitted for submits, cancels, and fills with no realized PnL. Fees are exposed separately and are not netted into this value. |
| `orders[].transactions[].liquidationFee` | string? | Liquidation fee in USDC charged on this fill, reported by `LiquidationLog` (only for liquidation fill transactions). The fee is capped on-chain at the account's remaining equity, so underwater accounts are charged nothing and report `"0"`. Rows indexed before 2026-07-08 report the pre-cap accrued maximum instead, which may exceed the amount actually deducted. |
| `orders[].transactions[].role` | string? | `"maker"` or `"taker"` (only for `"fill"` transactions) |
| `orders[].transactions[].reason` | string? | Why the order was removed (only for `"cancel"` transactions), as the WebSocket `Cancel` event's `reason` values |
| `orders[].transactions[].tpslId` | number? | `internalId` of the take-profit / stop-loss whose triggered order displaced this one (only with `reason: "TpslTriggerPriority"`) |
| `orders[].transactions[].tpslKind` | string? | `"TakeProfit"` or `"StopLoss"` — which kind of setting displaced it, alongside `tpslId` |

**Example:**
```bash
# Get all order history
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory

# Get filled orders only
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?status=filled"

# Get orders for specific market with pagination
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?symbol=SOL&limit=50&offset=100"

# Get orders in a time range
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/orderHistory?startTime=1702656000&endTime=1702742400"
```

---

### Get Transaction History

Get unified collateral-movement history (deposits, withdrawals, collateral transfers, fee rebates, and crosschain transfers made through the Gum app) for a trading account.

> **Settlement flows are not included.** `settle_pnl` and `settle_funding` move value *within* an account rather than in or out of it, so they are not transactions in this sense. Use [Get User Funding History](#get-user-funding-history) for settled funding payments.

> **Family aggregation.** Querying a cross-margin subaccount also returns the rows of its isolated-margin accounts (same user and `subaccountId`). Querying an isolated account directly returns only its own rows.

> Collateral moves **between** accounts in the same family are omitted: they are internal to the aggregated view and would otherwise appear twice, once as `transfer_in` and once as `transfer_out`.

**Endpoint:** `GET /tradingAccount/{trading_account}/transactionHistory`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `trading_account` | string | Trading account public key |

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `transactionType` | string | No | Filter by type: `"deposit"`, `"withdraw"`, `"transfer_in"`, `"transfer_out"`, `"crosschain_deposit"`, `"crosschain_withdraw"`, `"crosschain_withdraw_refund"`, `"fee_rebate"` |
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
      "transactionType": "withdraw",
      "amount": "-500.000000",
      "signature": "7gzmu6WuHx0R0PLfVSFr3wbLsMs359fx4P1f",
      "timestamp": 1702656200
    },
    {
      "transactionType": "transfer_out",
      "amount": "-100.000000",
      "signature": "8h0nv7XvIy1S1QMgWTGs4xcMtNt460gy5Q2g",
      "timestamp": 1702656300
    },
    {
      "transactionType": "fee_rebate",
      "amount": "12.500000",
      "signature": "9j1ow8YwJz2T2RNhXUHt5ydNuOu571hz6R3h",
      "timestamp": 1702656400
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `transactions` | array | Array of transaction history entries |
| `transactions[].transactionType` | string | Transaction type: `"deposit"`, `"withdraw"`, `"transfer_in"`, `"transfer_out"`, `"crosschain_deposit"`, `"crosschain_withdraw"`, `"crosschain_withdraw_refund"`, `"fee_rebate"` |
| `transactions[].amount` | string | Signed amount in USDC (positive = credit, negative = debit) |
| `transactions[].signature` | string | Transaction signature |
| `transactions[].timestamp` | number | Unix timestamp (seconds) |

**Notes:**
- Amounts use fixed-point formatting with 6 decimal places (USDC decimals)
- Withdrawals are shown as negative amounts
- Collateral transfers (via `/transfer`) appear on both sides: `transfer_out` with a negative amount on the source account and `transfer_in` with a positive amount on the destination account
- `crosschain_deposit`, `crosschain_withdraw` and `crosschain_withdraw_refund` are bridge transfers made through the Gum app. They always post to the cross-margin account, and their `signature` is not one the account owner signed
- `fee_rebate` is a market maker fee rebate paid into the account (positive amount); its `signature` is not one the account owner signed. It is listed as `deposit` for about a minute after it lands, until the payout is linked to it
- Results are sorted by timestamp descending

**Example:**
```bash
# Get all transaction history
curl https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory

# Get only deposits
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?transactionType=deposit"

# Get transactions with pagination
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?limit=50&offset=100"

# Get transactions in a time range
curl "https://api.gum.ag/perps/tradingAccount/YOUR_TRADING_ACCOUNT/transactionHistory?startTime=1702656000&endTime=1702742400"
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
- Reserved-band cross-margin accounts (`subaccountId` 200–255) reject direct deposits with `400 BAD_REQUEST`; their isolated children remain valid targets

**Example:**
```bash
curl "https://api.gum.ag/perps/deposit?amount=1000000&trading_account=YOUR_TRADING_ACCOUNT" \
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
curl -X POST https://api.gum.ag/perps/deposit \
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
| `trading_account` | string | Yes | Source trading account public key (cross-margin only) |

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
- `trading_account` must be a cross-margin account. An isolated-margin account is rejected with `400 BAD_REQUEST` (the contract also rejects it); transfer the collateral to the cross-margin account first, then withdraw

**Example:**
```bash
curl "https://api.gum.ag/perps/withdraw?amount=1000000&trading_account=YOUR_TRADING_ACCOUNT" \
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
curl -X POST https://api.gum.ag/perps/withdraw \
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
curl "https://api.gum.ag/perps/transfer?amount=1000000&source_trading_account=SOURCE_TRADING_ACCOUNT&destination_trading_account=DESTINATION_TRADING_ACCOUNT" \
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
curl -X POST https://api.gum.ag/perps/transfer \
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
curl https://api.gum.ag/perps/blockhash
```

---

## Health

### Health Check

Check the health status of the API service, its dependencies, and each listed market's orderbook liquidity. Market health is reported independently: an unhealthy market does not change the overall status, HTTP status code, or trading availability.

**Endpoint:** `GET /health`

**Response:**
```json
{
  "status": "healthy",
  "trading_available": true,
  "ohlcv_available": true,
  "degraded_services": [],
  "market_health": [
    {
      "symbol": "SOL",
      "status": "unhealthy",
      "depth_status": "thin",
      "bid_depth_usd": 5000.0,
      "ask_depth_usd": 15000.0
    }
  ]
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Overall status: `"healthy"`, `"degraded"`, or `"unhealthy"` |
| `trading_available` | boolean | Whether trading functionality is available (requires a fresh cached blockhash) |
| `ohlcv_available` | boolean | Whether OHLCV/candlestick data is available (requires the historical data store) |
| `degraded_services` | array | List of unavailable services: `"redis"`, `"clickhouse"`, `"blockhash"`, `"postgres"` |
| `market_health` | array | Liquidity health for every market listed by `/markets`, sorted by symbol; empty when no markets are cached |

**Market Health Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | string | Market symbol |
| `status` | string | `"healthy"` or `"unhealthy"`; only normal depth is healthy |
| `depth_status` | string | `"normal"`, `"thin"`, `"empty"`, `"one_sided"`, or `"unavailable"` |
| `bid_depth_usd` | number or null | Bid notional within the depth band; null when unavailable |
| `ask_depth_usd` | number or null | Ask notional within the depth band; null when unavailable |

Depth is computed from the best 500 aggregated price levels per side and sum price × size for bids at or above 99% of the midpoint and asks at or below 101%. If a two-sided midpoint is not available, use the market's mark price. Both sides must contain at least $10,000 in the band for `normal` depth; exactly $10,000 passes. Otherwise, a two-sided book is `thin`; a book with no orders is `empty`, and a book with orders on only one side is `one_sided`.

Missing orderbook data or a reference price that is not finite and positive reports `status: "unhealthy"`, `depth_status: "unavailable"`, and null depths. This check uses cached snapshots without a separate freshness rule and does not restrict trading.

**Status Codes:**

| Status Code | Condition |
|-------------|-----------|
| 200 OK | All services healthy or degraded (core functionality available) |
| 503 Service Unavailable | Critical services unavailable (cache down or cached blockhash stale) |

**Status Meanings:**

| Status | Description |
|--------|-------------|
| `healthy` | All services operational |
| `degraded` | Core trading works, but the historical data store and/or database is unavailable |
| `unhealthy` | The cache is unavailable or the cached blockhash has expired; trading not possible |

**Example:**
```bash
curl https://api.gum.ag/perps/health
```

---

## WebSocket

### WebSocket Endpoint

Connect to the WebSocket endpoint for real-time updates.

**Endpoint:** `GET /ws` (WebSocket upgrade)

See [WEBSOCKET_API.md](./WEBSOCKET_API.md) for WebSocket documentation.

---

## Error Responses

Application errors represented by `ApiError` use the following format. Framework-level request rejections (for example malformed JSON/query fields or a router timeout) may use a different body:

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
| `CONFLICTING_MARGIN_TYPE` | 400 | No | A cross-margin order was placed on a market where the signer's isolated-margin account still holds a position or a resting order (see "One margin type per market") |
| `DUPLICATE_SIGNATURE` | 409 | No | The same signed trading action was already accepted; sign a fresh action with a new blockhash instead of resending (see "Signing Trading Actions") |
| `RATE_LIMITED` | 429 | Yes | The signing wallet sent order actions faster than its rate limit (see "Order action rate limit"); retry after the `Retry-After` delay |
| `RPC_UNAVAILABLE` | 503 | Yes | Solana RPC node connection unavailable |
| `SERVICE_DEGRADED` | 503 | Yes | A backend dependency is temporarily unavailable |
| `INVALID_TRANSACTION` | 400 | No | Transaction contains invalid data |
| `TRANSACTION_EXPIRED` | 400 | No | Recent blockhash has expired or is unknown; fetch a new blockhash and sign again |
| `TRANSACTION_TIMEOUT` | 504 | Yes | Transaction confirmation timed out |
| `TRANSACTION_FAILED` | 500 | No | Transaction failed on-chain |
| `INTERNAL` | 500 | No | Internal server error |

**Retry-After Header:**

For retryable errors (429, 503, 504), the response includes a `Retry-After` header indicating how many seconds to wait before retrying: `1` for `429`, `5` otherwise.

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
| 429 | Too Many Requests - Order action rate limit exceeded; retry after `Retry-After` |
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
| POST | `/conditionalOrder` | Arm a take-profit / stop-loss setting |
| POST | `/conditionalOrder/cancel` | Clear one or every armed setting on a symbol |
| POST | `/conditionalOrder/update` | Re-arm one live setting in place |
| GET | `/leverage` | Build set-leverage transaction |
| POST | `/leverage` | Submit signed set-leverage transaction |
| GET | `/tradingAccount` | List user's accounts |
| POST | `/tradingAccount` | Create trading account |
| POST | `/tradingAccount/{trading_account}/name` | Rename subaccount |
| GET | `/tradingAccount/{trading_account}/orders` | Get all open orders |
| GET | `/tradingAccount/{trading_account}/conditionalOrders` | Get armed take-profit / stop-loss settings |
| GET | `/tradingAccount/{trading_account}/positions` | Get perpetual positions |
| GET | `/tradingAccount/{trading_account}/leverage` | Get per-market leverage preferences |
| GET | `/tradingAccount/{trading_account}/balances` | Get token balances |
| GET | `/tradingAccount/{trading_account}/trades` | Get trade history |
| GET | `/tradingAccount/{trading_account}/orderHistory` | Get order history |
| GET | `/tradingAccount/{trading_account}/transactionHistory` | Get transaction history |
| GET | `/deposit` | Build deposit transaction |
| POST | `/deposit` | Submit signed deposit |
| GET | `/withdraw` | Build withdraw transaction |
| POST | `/withdraw` | Submit signed withdraw |
| GET | `/transfer` | Build collateral transfer between cross subaccounts or between a cross account and its isolated child (either direction) |
| POST | `/transfer` | Submit signed transfer |
| GET | `/blockhash` | Get recent blockhash |
| GET | `/health` | Health check |
| GET | `/ws` | WebSocket endpoint |
