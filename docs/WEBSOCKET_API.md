# WebSocket API Documentation

## Connection

Connect to the WebSocket endpoint:

```
wss://api.gum.ag/perps/ws
```

## Limits

Each API instance enforces these limits; the defaults are shown and operators can change them. Per-connection limits are ceilings that stop a single client from monopolizing an instance. They are not a throughput guarantee: when many clients are busy at once, the per-instance budgets are reached first.

| Limit | Default | When exceeded |
|-------|---------|---------------|
| Open connections per API instance | 2,000 | The upgrade is answered with HTTP `503` `Too many WebSocket connections`; retry, ideally with backoff |
| Inbound message size | 16 KiB | The connection is dropped |
| Inbound messages per connection | 2,000/s sustained, bursts of 5,000 | That message is rejected |
| Inbound messages per API instance, across connections | 8,000/s | Reading pauses until the instance has capacity: messages are delayed, not rejected |
| In-flight `post` requests per connection | 512 | That `post` is rejected |
| In-flight `post` requests per API instance | 1,024 | That `post` is rejected |
| Subscriptions per connection | 200 | That `subscribe` is rejected |
| Request `id` length | 128 bytes | The message is rejected |
| Subscription `symbol` length | 32 bytes | The message is rejected |
| Outbound messages queued for a client that is not reading | 1,024 messages or 4 MiB | The connection is closed |
| Outbound bytes queued across all clients of an API instance | 1 GiB | While over it, each client holding more than its share (1 GiB ÷ the connection cap, 512 KiB by default) is closed on its next message |

Every message except a close frame counts toward the message rate, including pings and malformed messages.

A rejected message gets the reply shape its method already uses, and the connection stays open:

| Rejected method | Reply |
|-----------------|-------|
| `post`, `get` | `type: "request"` with `success: false`, the request `id`, and the reason in `error` |
| `subscribe`, `unsubscribe` | `type: "subscription"` with `success: false` and the reason in `error` |
| Request `id` or `symbol` too long | `type: "error"` with the reason in `message`; the oversized field is not echoed |

Reasons start with a stable prefix:

- `Rate limit exceeded`
- `Too many in-flight requests` (per connection)
- `Server busy` (per API instance)
- `Subscription limit reached`
- `Field too long`

The server closes the connection with close code `1008` (policy violation) in two cases:

- **`Too many rejected requests`:** the connection collected 1,000 rejections within 10 seconds. `Server busy` rejections don't count toward this.
- **`Slow consumer`:** its outbound queue overflowed. Messages still queued are discarded, so reconnect and resubscribe to resynchronize.

## Message Format

Client messages use JSON format with a `method` field as the discriminator.
Server messages are enveloped with `type`, `data`, and `ts` — the server machine timestamp in milliseconds since the Unix epoch, stamped when the message is sent:

```json
{
  "type": "blockhash",
  "data": {
    "blockhash": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
  },
  "ts": 1756339200123
}
```

The subscription data examples below show the `data` object unless the full envelope is shown, and omit `ts` for brevity — every server message carries it.

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

> **Subscribe to the cross-margin subaccount.** One subscription covers a subaccount and every isolated-margin account under it: an isolated account's events are delivered to its cross-margin parent, and subscribing to the isolated account itself receives nothing. Each event payload still names the account it came from, so isolated activity remains distinguishable. The same rule applies to the positions and open-orders subscriptions below.

> **Events from a server restart arrive late, not missing.** After a restart, the server republishes the slots it missed before resuming live delivery. Those events can therefore arrive several seconds after their `timestamp`, and after newer events if they cannot be fetched at first. On a connection that stays open, each event is still delivered once, including during a rolling restart. Only the last three minutes or so before the restart are replayed, so re-read the REST state after a longer outage. Events emitted while your own connection is down are not redelivered when you reconnect. The same applies to the trades subscription.

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
| `MoveCollateral` | Collateral moved between two cross-margin subaccounts or between a cross-margin account and its isolated child. Sent once to each distinct source/destination family root. |
| `LeverageUpdate` | The account's per-market leverage preference changed. Sent to subscribers of the trading account whose preference was set. |
| `ConditionalOrderUpsert` | A take-profit / stop-loss setting was armed, or an armed one was re-armed with new terms. |
| `ConditionalOrderCancel` | A setting was cleared — by a cancel request, by the position-gone sweep, or after repeated on-chain rejections. Not emitted when a setting fires. |
| `ConditionalOrderTrigger` | A setting's trigger price was crossed and its reduce-only order was matched. |
| `CrosschainDeposit` | A bridge deposit made through the Gum app was credited to the account. |
| `CrosschainWithdraw` | A bridge withdrawal made through the Gum app was debited from the account. |
| `CrosschainIntent` | A Gum app crosschain intent executed for the account. |
| `CrosschainRefundDeposit` | A Gum app crosschain intent deposit was refunded. |
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
  "clientOrderId": "12345",
  "side": "Ask",
  "timestamp": 1702656000,
  "expiryTimestamp": 0,
  "orderType": "limit",
  "timeInForce": "gtc",
  "reduceOnly": false,
  "collateralAmount": 500000000
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
| `clientOrderId` | string | Client-provided order ID as a decimal string (`"0"` if not set) |
| `side` | string | Order side: `"Bid"` or `"Ask"` |
| `timestamp` | number | Unix timestamp (seconds) |
| `expiryTimestamp` | number | Expiry timestamp (0 = no expiry) |
| `orderType` | string | Order type: `"limit"`, `"market"`, or `"postOnly"` |
| `timeInForce` | string | Time in force: `"gtc"`, `"gtd"`, `"ioc"`, `"fok"`, `"liquidation"`, or `"adl"` |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |
| `collateralAmount` | number | USDC (native units) the **isolated** order committed from its cross-margin parent as sent by the client — the budget backing its position margin and resting reservation, and the only explanation anywhere for the parent's balance dropping at placement. `0` for cross-margin and reduce-only orders (a reduce-only order sending a budget is rejected). Reported as submitted; the amount actually drawn is recorded separately on-chain (a pure reduction takes nothing), and `orderHistory` reports the split as `initialCollateral`/`executedCollateral`. |

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
  "clientOrderId": "12345"
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
| `clientOrderId` | string \| null | Client-provided order ID as a decimal string (maker's when sent to maker, taker's when sent to taker) |

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
  "clientOrderId": "12345"
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
| `clientOrderId` | string \| null | Client-provided order ID as a decimal string, if one was set on the cancelled order |
| `tpslId` | number? | `internalId` of the take-profit / stop-loss that displaced this order. Only on `reason: "TpslTriggerPriority"` |
| `tpslKind` | string? | `"TakeProfit"` or `"StopLoss"` — which kind of setting displaced it, alongside `tpslId` |

**`reason` values:**

| Value | When |
|-------|------|
| `"UserRequest"` | Explicit cancel by the order owner |
| `"CancelAll"` | Owner cancelled all orders for the market |
| `"SelfTradePrevention"` | Owner placed a new order that would have matched their own resting order |
| `"Expired"` | Order's `expiryTimestamp` elapsed; sent within a few seconds of expiry, when the order is removed from the book |
| `"ImmediateOrCancel"` | Unfilled remainder of an IOC (or liquidation) order |
| `"MatchLimitReached"` | Matching halted before fill due to compute/heap limits, so the remainder was not posted |
| `"Eviction"` | Orderbook full — the worst-priced order was evicted to make room for an incoming better-priced one |
| `"ReduceOnlyInvalid"` | Existing reduce-only order was invalidated by a same-side order placement |
| `"TpslTriggerPriority"` | A fired take-profit / stop-loss outranked this resting reduce-only order, which expired to make room for the trigger. `tpslId` and `tpslKind` name the setting |
| `"Liquidation"` | Cancelled during a liquidation cascade for the owner |
| `"PriceBandExceeded"` | Resting order drifted outside the price-deviation band and was expired when a taker reached it |
| `"InsufficientMargin"` | Expired by the margin keeper: the owner fell below initial margin, this order would have grown the position, and the mark had moved through its limit |
| `"Invalid"` | SDK fallback for an unknown numeric cancellation reason |

#### MoveCollateral Event

Emitted when the `move_collateral` instruction moves USDC between two cross-margin subaccounts or between a cross-margin subaccount and its isolated child. Each distinct source/destination family root receives one copy; a move within one family is not delivered twice.

```json
{
  "action": "MoveCollateral",
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
| `direction` | string | `"CrossToIsolated"`, `"IsolatedToCross"`, or `"CrossToCross"` |
| `timestamp` | number | Unix timestamp (seconds) |

#### LeverageUpdate Event

Emitted when `POST /leverage` sets a per-market leverage preference. A live position in that market adopts the new value immediately, so its `selectedLeverage`, `effectiveImrBps` and `margin` change with it; the preference also applies to later orders.

```json
{
  "action": "LeverageUpdate",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "leverageConfig": "ABC123...",
  "market": "SOL",
  "marketPubkey": "DEF456...",
  "tradingAccount": "GHI789...",
  "timestamp": 1702656000,
  "subaccountId": 0,
  "isIsolated": false,
  "oldLeverage": 0,
  "newLeverage": 5
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `leverageConfig` | string | Leverage config account public key |
| `market` | string | Market symbol (e.g., `"SOL"`). Falls back to the market public key if the symbol cannot be resolved. |
| `marketPubkey` | string | Market public key |
| `tradingAccount` | string | Trading account the preference belongs to |
| `timestamp` | number | Unix timestamp (seconds) |
| `subaccountId` | number | Subaccount index |
| `isIsolated` | boolean | `true` when the isolated-margin preference was set |
| `oldLeverage` | number | Previous leverage (0 = was unset) |
| `newLeverage` | number | New leverage |

#### ConditionalOrderUpsert Event

Emitted when a take-profit / stop-loss setting is armed, and again whenever
`POST /conditionalOrder/update` re-arms it — the same fields either way, always
the terms the setting now holds. Keyed on `internalId` it reads as an upsert, so
a client that applies every one of these ends up correct without branching;
`isUpdate` is there for clients that want to tell the first version apart.

```json
{
  "action": "ConditionalOrderUpsert",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "conditionalOrders": "ABC123...",
  "market": "SOL",
  "marketPubkey": "GHI789...",
  "tradingAccountId": 42,
  "internalId": 3,
  "triggerPriceLots": 12000,
  "triggerInputValue": 12000,
  "triggerInputKind": "price",
  "closeAmount": 5000,
  "closeAmountKind": "bps",
  "limitPriceLots": 0,
  "clientTpslId": 77,
  "positionId": 3,
  "timestamp": 1702656000,
  "kind": 0,
  "triggerDirection": 1,
  "positionSide": 0,
  "triggerPriceType": 0,
  "isUpdate": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `conditionalOrders` | string | `ConditionalOrders` account, derived per `(trading account, market)` |
| `market` | string | Market symbol. Falls back to the market public key if the symbol cannot be resolved. |
| `marketPubkey` | string | Market public key |
| `tradingAccountId` | number | Trading account ID the setting belongs to |
| `internalId` | number | Addresses the setting for its whole life; carried by the cancel and trigger events |
| `triggerPriceLots` | number | Trigger price in quote lots per base lot |
| `triggerInputValue` | number | What the user entered, read according to `triggerInputKind`. The trigger itself is always `triggerPriceLots` |
| `triggerInputKind` | string | `"price"` (quote lots per base lot), `"pnl"` (native quote), or `"roiBps"` — the create's `roiPercent` converted, `10%` reading back as `1000`. The latter two are magnitudes; `kind` gives the direction |
| `closeAmount` | number | Close size, read according to `closeAmountKind` |
| `closeAmountKind` | string | `"lots"` (fixed at create), `"bps"` (re-measured against the position at trigger), or `"notional"` (native quote units, divided by the trigger price) |
| `limitPriceLots` | number | Limit price of the submitted order; `0` = market-after-trigger |
| `clientTpslId` | number | Client id the setting was armed under (`0` if none) |
| `positionId` | number | The position instance the setting is bound to. A setting whose position closed or flipped is cleared, not fired; a liquidation the position survives keeps it |
| `timestamp` | number | Unix timestamp (seconds) |
| `kind` | number | `0` = take profit, `1` = stop loss |
| `triggerDirection` | number | `0` = at-or-below, `1` = at-or-above |
| `positionSide` | number | `0` = long, `1` = short |
| `triggerPriceType` | number | Price the trigger watches: `0` = mark, `1` = last |
| `isUpdate` | boolean | `true` when an update re-armed a setting that already existed, `false` on its first arming |

Prices and sizes are in lots here, unlike the REST listing's decimals.

#### ConditionalOrderCancel Event

Emitted when a setting is cleared, one event per setting. A setting that **fires** is
cleared without one of these — `ConditionalOrderTrigger` is its record.

```json
{
  "action": "ConditionalOrderCancel",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "conditionalOrders": "ABC123...",
  "market": "SOL",
  "marketPubkey": "GHI789...",
  "tradingAccountId": 42,
  "internalId": 3,
  "timestamp": 1702656000,
  "kind": 0,
  "reason": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `internalId` | number | The setting that was cleared |
| `kind` | number | `0` = take profit, `1` = stop loss |
| `reason` | number | `0` = user request, `1` = position gone (closed, reversed, taken over by a liquidation, a later instance, or the close it resolves to is below the market minimum), `2` = the trigger was rejected on-chain 5 times running, `3` = the limit-after-trigger price is outside the market's price band |

Other fields are as on `ConditionalOrderUpsert`.

#### ConditionalOrderTrigger Event

Emitted when the trigger price is crossed and the reduce-only order it submits has
been matched.

```json
{
  "action": "ConditionalOrderTrigger",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "conditionalOrders": "ABC123...",
  "market": "SOL",
  "marketPubkey": "GHI789...",
  "tradingAccountId": 42,
  "internalId": 3,
  "observedPricePerLot": 12010000,
  "triggerPricePerLot": 12000000,
  "submittedBaseLots": 50,
  "filledBaseLots": 50,
  "orderId": 123456789,
  "clientTpslId": 77,
  "timestamp": 1702656000,
  "kind": 0,
  "triggerPriceType": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `internalId` | number | The setting that fired |
| `observedPricePerLot` | number | The price that crossed the trigger, from the source `triggerPriceType` names, in native quote per base lot |
| `triggerPricePerLot` | number | The trigger price in the same units, for comparison |
| `submittedBaseLots` | number | Size submitted, after a percent size was re-measured or a fixed size clipped |
| `filledBaseLots` | number | How much filled. A short fill is **not** retried — the setting is gone either way |
| `orderId` | number | Sequence number of the submitted order. Unlike `orderId` elsewhere in this document it is not the composite `(price, seq)` id, so it cannot be passed to a cancel request |
| `clientTpslId` | number | Client id the setting was armed under (`0` if none) |
| `kind` | number | `0` = take profit, `1` = stop loss |
| `triggerPriceType` | number | `0` = mark, `1` = last |

Other fields are as on `ConditionalOrderUpsert`. If the fill closed the position
outright, every setting still armed on that market is swept alongside it, each
emitting a `ConditionalOrderCancel` with `reason: 1`.

#### Error Event

Error events are sent when an order is rejected or fails on-chain (e.g., insufficient margin, post-only order would match). Gum app crosschain transfers that fail on-chain also arrive here, on the cross-margin subaccount they targeted.

```json
{
  "action": "Error",
  "signature": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
  "error": "PostOnlyOrderWouldMatch",
  "clientOrderId": "12345"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `signature` | string | Transaction signature |
| `error` | string | Error message (e.g., `PostOnlyOrderWouldMatch`, `OrderNotFound`, `InsufficientMargin`) |
| `clientOrderId` | string \| null | Client-provided order ID as a decimal string, if one was included in the original request |

---

### Positions Subscription

Subscribe to receive real-time position updates for a specific trading account. Subscribing to a cross-margin subaccount also covers its isolated-margin accounts' positions, matching `GET /tradingAccount/{trading_account}/positions`. A full snapshot is broadcast every ~2 seconds while positions exist, and per-position updates fire on each fill the account participates in. When a fill closes a position, a zero-size update (`size: "0"`, `avgEntryPrice: "0"`, etc.) is emitted on the closed market so clients can clear local state — REST `GET /tradingAccount/{trading_account}/positions` continues to omit closed positions.

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
| `liquidationPrice` | string \| null | Estimated liquidation price (`null` when no estimate is available, including a closed-position update). Cross-margin positions account for the account's shared collateral; isolated positions are priced against the isolated account's own collateral |
| `fundingAccrued` | string | Unsettled funding accrued on this position (USDC). Positive = funding received, negative = funding paid |
| `pendingFundingPayable` | string | Pending funding this position still owes (USDC, >= 0). Mirrors the negative side of `fundingAccrued` |
| `pendingFundingReceivable` | string | Pending funding this position is still owed (USDC, >= 0). A pending receivable only — not claimable, transferable, or withdrawable until actually settled |
| `totalFunding` | string | Total funding this position has accrued since it was opened (USDC): payments already settled to collateral plus the pending amount. Positive = funding received, negative = funding paid. Starts from zero when the position opens from flat or flips side; adding to or partially closing the position keeps the running total. A position already open when the program began tracking it counts from the program upgrade onward |
| `selectedLeverage` | number | Leverage currently applied to the position, updated in place by `POST /leverage` (0 = none selected) |
| `effectiveImrBps` | number | Initial margin ratio applied to this position in bps; the market ratio when no leverage is selected. Never 0, including on zero-size updates |
| `maintenanceMarginRatioBps` | number | Maintenance margin ratio applied to this position in bps |
| `maintenanceAmount` | string | Tier deduction keeping maintenance continuous with the band below (USDC). Maintenance margin is `notional × maintenanceMarginRatioBps / 10000 - maintenanceAmount`, floored at 0 |
| `tierIndex` | number \| null | Band the position's notional resolves to, indexed into the tier table. `null` for legacy positions and untiered markets, which margin on the market's flat ratios |
| `tierTableIdx` | number | Tier table this position margins through. `0` is the market default returned by `/markets`; a whitelisted account can be pinned to another table |
| `isolated` | boolean | `true` when the position is held in an isolated-margin account |
| `collateral` | string \| null | For an isolated position, the USDC collateral held by its isolated account — the equity backing the position (deposits ± realized PnL and fees, whether or not yet settled, excluding the open lots' unrealized PnL and funding). Can be negative while a bankrupt position is being liquidated. `null` for cross-margin positions |
| `adlQuantile` | number \| null | ADL indicator: where the position stands in the market's auto-deleveraging queue for its side, which ranks profitable positions by profit percentage × effective leverage. `4` = top 10% (the head of the queue is always `4`), `3` = top 25%, `2` = top 50%, `1` = the rest, `0` = not in the queue (not profitable, account under liquidation, or ADL disabled on the market; see `isAdlEnabled` on `/markets`). Re-ranked about every 5 seconds; `null` while the indicator is unavailable. Current standing only, not a prediction that the position will or will not be deleveraged. `0` on a closed-position update |

> **Isolated accounts.** Subscribe to the cross-margin subaccount: its stream includes the positions held by its isolated-margin accounts (each marked `isolated: true`), matching the REST positions endpoint. Subscribing to an isolated account's own pubkey receives nothing.

---

### Open Orders Subscription

Subscribe to receive real-time updates for a trading account's open orders. Subscribing to a cross-margin subaccount also covers orders resting on its isolated-margin accounts, matching `GET /tradingAccount/{trading_account}/orders`. Each order update is sent individually as changes occur.

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
  "clientOrderId": "12345",
  "reduceOnly": false,
  "isolated": false,
  "collateral": null
}
```

*(`collateral` is `null` above because `isolated` is `false`; an isolated order carries a USD string such as `"200"`.)*

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
| `status` | string | Order status: `"open"`, `"partially_filled"`, `"filled"`, `"cancelled"` (nothing filled), `"partially_filled_cancelled"` (cancelled after a partial fill) |
| `clientOrderId` | string | Optional client-provided order ID as a decimal string (omitted if not set) |
| `reduceOnly` | boolean | Whether the order can only reduce an existing position |
| `isolated` | boolean | True when the order rests on an isolated-margin account; cancelling it requires `isIsolated: true` |
| `collateral` | string \| null | For an isolated order, the USDC collateral the chain currently reserves against it (USD), matching `GET /tradingAccount/{trading_account}/orders`. Released proportionally as the order fills, so it tracks `remainingSize` rather than the amount funded at placement. `null` for cross-margin orders |

> **An isolated order update is never sent without its collateral.** The value is read from the on-chain account snapshot, which is written by a pipeline independent of the update that triggers this push. When the snapshot has not caught up, the read is retried briefly; if it still cannot be resolved, **the update is dropped rather than sent with `collateral: null`** — a null there is indistinguishable from a cross-margin order and would misreport the margin mode. A dropped update is recovered on that order's next update, or by re-reading `GET /tradingAccount/{trading_account}/orders`; treat the REST listing as authoritative after any reconnect. A `"filled"` or `"cancelled"` update needs no snapshot read — the chain has already released the reservation, so it always delivers with `collateral: "0"`. Cross-margin order updates are unaffected and always sent.

> **Residual staleness.** The snapshot can be momentarily *ahead* of the notification, pairing a newer reservation with the `remainingSize` from the update that triggered the push. It self-corrects on the next update for that order.

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
  "time": 1702656000,
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
  "asks": [[50001.0, 1.0], [50002.0, 3.0]],
  "timestamp": 1702656000,
  "lastUpdatedMs": 100
}
```

- `bids`: Array of `[price, size]` sorted by price descending (best bid first)
- `asks`: Array of `[price, size]` sorted by price ascending (best ask first)
- `timestamp`: On-chain snapshot timestamp (Unix seconds)
- `lastUpdatedMs`: Milliseconds since the orderbook cache was updated

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
| `markets[].fundingRateBps1h` | number \| null | The market's latest on-chain funding rate scaled to one hour (bps); `null` only for a market without a funding interval |
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

Trading actions require a signed payload. The `id` field is returned in the response for request correlation. REST validation rules also apply, but a rejected WebSocket action returns a `type: "request"` message with `success: false` and an `error` string, not an HTTP 400 response.

Every signed action is single-use: resending an accepted action is rejected with `success: false` and the error `Duplicate signature` (the REST `409 DUPLICATE_SIGNATURE`) while the signature is still tracked (~45 s), and dropped silently by the sequencer after that. Watch the outcome on `tradingAccountEvents` / `openOrders` instead of resending, and sign a fresh action with a new blockhash when you need to act again.

Signed actions share the per-wallet order action rate limit with REST (see "Order action rate limit" in [REST_API.md](REST_API.md)). An action over the limit gets `success: false` with the error `Rate limit exceeded: too many order actions for this wallet` (the REST `429 RATE_LIMITED`).

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
      "clientOrderId": "12345",
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
| `subaccountId` | number | No | Subaccount index (0-255). Defaults to `0`; cross-margin placement rejects reserved ids `200..=255`, while isolated placement remains allowed |
| `symbol` | string | Yes | Market symbol (e.g., "SOL") |
| `side` | string | Yes | `"Bid"`/`"Ask"`; lowercase aliases `"bid"`/`"ask"` and `"buy"`/`"sell"` are also accepted |
| `orderType` | string | No | `"limit"` (default), `"postOnly"`, or `"market"` |
| `price` | number | Conditional | `> 0` for `limit` and `postOnly`. For `market`: omitted/`0` sweeps at any price; `> 0` acts as a slippage cap |
| `quantity` | number | Yes | Quantity in base units (e.g., 1.5) |
| `reduceOnly` | boolean | No | If true, only reduce existing position |
| `timeInForce` | string/object | No | Defaults to `"ImmediateOrCancel"` for `market` and `"GoodTilCanceled"` for `limit`/`postOnly`. Market orders accept `IOC` or `FOK`; post-only orders reject `IOC` and `FOK` |
| `clientOrderId` | string \| number | No | Optional client-provided order ID. Send it as a decimal string: IDs above 2^53 lose precision when a JS client parses a JSON number. A number is still accepted |
| `isIsolated` | boolean | No | Route the order to an **isolated-margin** account (the `place_order_isolated` instruction). Must be paired with `collateralAmount`; defaults to `false` (cross-margin) |
| `collateralAmount` | number | When `isIsolated` | USDC native units moved from the cross-margin parent into the isolated subaccount alongside the order; `0` moves no fresh collateral (the order draws on the isolated account's existing position/reservation). Must be paired with `isIsolated`, and must be `0` for a reduce-only order. The isolated account does **not** need to exist yet — the API creates it automatically before accepting the order |
| `tradingAccount` | string | Yes | The user's cross-margin subaccount, for isolated orders too |
| `recentBlockhash` | string | Yes | Recent blockhash for signing |
| `signature` | string | Yes | User's signature |

> **Isolated vs cross margin.** `isIsolated: true` with a `collateralAmount` (even `0`) places an isolated-margin order (the `place_order_isolated` instruction): the filled portion is backed by the isolated account's own collateral, while any resting remainder is fenced against the collateral reserved for the order — it is *not* backed by the rest of the isolated account's balance. An isolated order whose reservation is too small for its notional is rejected on-chain with `InsufficientMargin`. Omitting both fields places a plain cross-margin order; providing only one of the two is rejected with a request response containing `success: false` and an `error` string.
>
> **`tradingAccount` is the cross-margin subaccount in both modes.** The isolated account is derived server-side from the signing user, `subaccountId` and `symbol`, so clients never compute its address. Sending the isolated account instead is rejected with a request response containing `success: false` and an `error` string.
>
> Market orders use `price = 0` to sweep at any price; `> 0` acts as a slippage cap. Unfilled remainder is cancelled (IOC). Order history surfaces these as `orderType: "market"`; once fills exist, `price` is the weighted average execution price. An unfilled entry shows `"0"` (sweep) or the formatted cap.
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
| `isIsolated` | boolean | No | Target the isolated-margin account for `(subaccountId, symbol)` instead of the cross-margin subaccount. Defaults to `false`. `tradingAccount` stays the cross-margin subaccount either way; if no such isolated account exists the request is rejected with a request response containing `success: false` and an `error` string |
| `symbol` | string | Yes | Market symbol |
| `orderId` | string | No | Order ID to cancel (u128 as string) |
| `clientOrderId` | string \| number | No | Client order ID to cancel. Decimal string preferred; a number is still accepted |
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
| `isIsolated` | boolean | No | Target the isolated-margin account for `(subaccountId, symbol)` instead of the cross-margin subaccount. Defaults to `false`. `tradingAccount` stays the cross-margin subaccount either way; if no such isolated account exists the request is rejected with a request response containing `success: false` and an `error` string |
| `symbol` | string | Yes | Market symbol |
| `tradingAccount` | string | Yes | Trading account public key |
| `recentBlockhash` | string | Yes | Recent blockhash |
| `signature` | string | Yes | User's signature |

---

### Conditional Orders (TP/SL)

Create, update and cancel take-profit / stop-loss settings. The action body is the REST request body plus `method`; fields, validation and errors are those of [Create](REST_API.md#create-conditional-order), [Update](REST_API.md#update-conditional-order) and [Cancel Conditional Order](REST_API.md#cancel-conditional-order). The outcome streams on `tradingAccountEvents` as `ConditionalOrderUpsert` / `ConditionalOrderCancel`.

| `method` | REST equivalent |
|----------|-----------------|
| `"createConditional"` | `POST /conditionalOrder` |
| `"updateConditional"` | `POST /conditionalOrder/update` |
| `"cancelConditional"` | `POST /conditionalOrder/cancel` |

**Request:**
```json
{
  "method": "post",
  "id": "tpsl-123",
  "payload": {
    "pubkey": "USER_PUBKEY",
    "action": {
      "method": "createConditional",
      "symbol": "SOL",
      "kind": "TakeProfit",
      "trigger": {"type": "price", "value": 120.0},
      "triggerPriceType": "MarkPrice",
      "closeAmount": {"type": "bps", "value": 5000},
      "tradingAccount": "TRADING_ACCOUNT_PUBKEY",
      "recentBlockhash": "BLOCKHASH",
      "signature": "SIGNATURE"
    }
  }
}
```

A success response carries `{"success": true}` in `data`.

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
      "clientOrderId": "12345"
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
| `post` | `createConditional` | Arm a TP/SL |
| `post` | `updateConditional` | Re-arm a TP/SL |
| `post` | `cancelConditional` | Cancel TP/SL settings |
