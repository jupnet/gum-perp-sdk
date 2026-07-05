# gum-perp-sdk

Standalone Rust SDK for Gum Perps REST/WebSocket API and signed trading payloads.

```toml
[dependencies]
gum-perp-sdk = "0.1"
```

## Included

- Typed REST client for Gum Perps API endpoints.
- WebSocket client for subscriptions, ping, and signed trading posts.
- Local signing helpers for order actions.
- External signer/HSM flow via `message_to_sign(...)` and `signed_request(...)`.

## Order flow

1. Create `RestClient::new(user_pubkey)` — it targets the production Gum Perps API
   (`GUM_API_URL`) and sends `X-PUBKEY: user_pubkey` on every request. Use
   `RestClient::with_base_url(base_url, user_pubkey)` for staging or local development.
2. Fetch market metadata with `client.market("SOL")`.
3. Fetch a recent blockhash with `client.blockhash()`.
4. Build `PlaceOrder`, `CancelOrder`, or `CancelAllOrders`.
5. Sign locally with `sign_request_with(...)`, or ask an external signer to sign `message_to_sign(...)`.
6. Submit with `client.place_order(&signed)` or `WebsocketClient::post_signed(...)`. To
   batch, wrap signed place/cancel bodies in `BatchOrderActionSigned` and submit a
   `BatchOrderRequest` with `client.batch_orders(&batch)`.

For WebSocket, `WebsocketClient::connect()` targets production; `connect_to(base_url)`
takes a custom endpoint. The user pubkey must match the signer of trading actions.

See `examples/` for REST, signing, and WebSocket usage.
