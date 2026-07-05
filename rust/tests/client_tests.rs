use gum_perp_sdk::{Pubkey, RestClient, WebsocketClient, GUM_API_URL};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

async fn one_request_server(response_body: &'static str) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0_u8; 8192];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let _ = tx.send(request);
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });

    (format!("http://{addr}"), rx)
}

#[tokio::test]
async fn rest_client_sends_pubkey_header_and_decodes_markets() {
    let (base_url, request_rx) = one_request_server("[]").await;
    let pubkey = Pubkey::from([3_u8; 32]);
    let client = RestClient::with_base_url(base_url, pubkey).unwrap();

    let markets = client.markets().await.unwrap();

    assert!(markets.is_empty());
    let request = request_rx.await.unwrap();
    assert!(request.starts_with("GET /markets HTTP/1.1"), "{request}");
    assert!(
        request.contains(&format!("x-pubkey: {pubkey}")),
        "{request}"
    );
}

#[test]
fn websocket_client_derives_ws_url_from_http_base_url() {
    let url = WebsocketClient::websocket_url("https://api.example.com/jtx/").unwrap();

    assert_eq!(url.as_str(), "wss://api.example.com/jtx/ws");
}

#[test]
fn default_clients_target_the_production_gum_api() {
    let user = Pubkey::from([5_u8; 32]);

    let client = RestClient::new(user);
    assert_eq!(client.pubkey(), user);
    assert_eq!(
        client.endpoint_url("/markets"),
        "https://gum-api.jup.net/jtx/mainnet-beta/markets"
    );

    assert_eq!(
        WebsocketClient::websocket_url(GUM_API_URL).unwrap(),
        "wss://gum-api.jup.net/jtx/mainnet-beta/ws"
    );
}

// The API parses these queries with types that have NO camelCase rename
// (bin/api Query<CreateTradingAccountQuery> / Query<BuildTransactionQuery>);
// docs/REST_API.md documents `is_isolated` and `trading_account`. These tests
// pin the snake_case encoding so a well-meaning rename can't break the wire.
#[tokio::test]
async fn create_trading_account_query_encodes_snake_case_params() {
    use gum_perp_sdk::CreateTradingAccountQuery;

    let (base_url, request_rx) = one_request_server("{}").await;
    let client = RestClient::with_base_url(base_url, Pubkey::from([6_u8; 32])).unwrap();

    let _ = client
        .create_trading_account(&CreateTradingAccountQuery {
            is_isolated: true,
            symbol: Some("SOL".to_string()),
            subaccount_id: Some(2),
        })
        .await;

    let request = request_rx.await.unwrap();
    let request_line = request.lines().next().unwrap();
    assert!(request_line.contains("is_isolated=true"), "{request_line}");
    assert!(request_line.contains("subaccount_id=2"), "{request_line}");
    assert!(request_line.contains("symbol=SOL"), "{request_line}");
}

#[tokio::test]
async fn transfer_builder_encodes_snake_case_account_params() {
    let (base_url, request_rx) = one_request_server("{}").await;
    let client = RestClient::with_base_url(base_url, Pubkey::from([9_u8; 32])).unwrap();
    let source = Pubkey::from([10_u8; 32]);
    let destination = Pubkey::from([11_u8; 32]);

    let _ = client
        .build_transfer_transaction(250, &source, &destination)
        .await;

    let request = request_rx.await.unwrap();
    let request_line = request.lines().next().unwrap();
    assert!(request_line.starts_with("GET /transfer?"), "{request_line}");
    assert!(request_line.contains("amount=250"), "{request_line}");
    assert!(
        request_line.contains(&format!("source_trading_account={source}")),
        "{request_line}"
    );
    assert!(
        request_line.contains(&format!("destination_trading_account={destination}")),
        "{request_line}"
    );
}

#[tokio::test]
async fn deposit_builder_encodes_snake_case_trading_account_param() {
    let (base_url, request_rx) = one_request_server("{}").await;
    let client = RestClient::with_base_url(base_url, Pubkey::from([7_u8; 32])).unwrap();
    let trading_account = Pubkey::from([8_u8; 32]);

    let _ = client
        .build_deposit_transaction(150, &trading_account)
        .await;

    let request = request_rx.await.unwrap();
    let request_line = request.lines().next().unwrap();
    assert!(request_line.starts_with("GET /deposit?"), "{request_line}");
    assert!(request_line.contains("amount=150"), "{request_line}");
    assert!(
        request_line.contains(&format!("trading_account={trading_account}")),
        "{request_line}"
    );
}
