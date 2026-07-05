# Gum Perps SDK

Public SDKs and API documentation for the Gum Perps exchange (JTX), for market
makers and integrators.

## API Documentation

- [REST API](docs/REST_API.md) — market data, trading accounts, and signed
  trading endpoints (`/order/create`, `/order/cancel`, `/order/cancel-all`,
  `/order/batch`), plus deposit/withdraw/transfer flows.
- [WebSocket API](docs/WEBSOCKET_API.md) — subscriptions (blockhash, orderbook,
  trades, fills, …), read queries, and signed trading posts.

Production endpoint: `https://gum-api.jup.net/jtx/mainnet-beta` (WebSocket:
`wss://gum-api.jup.net/jtx/mainnet-beta/ws`).

## SDKs

| Language | Location | Status |
|----------|----------|--------|
| Rust | [`rust/`](rust/) | Available |
| Python | — | Planned |
| TypeScript | — | Planned |

## Quick start (Rust)

```toml
[dependencies]
gum-perp-sdk = "0.1"
```

The crate ships a typed REST client, a WebSocket client, and local signing
helpers for order actions, including a `message_to_sign(...)` flow for external
HSM/KMS signers. See the [crate README](rust/README.md) and
[`rust/examples/`](rust/examples/) for usage.
