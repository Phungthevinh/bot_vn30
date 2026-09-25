use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use vn30_domain::errors::MarketDataError;
use vn30_domain::market::{MarketEvent, Quote, Trade};
use vn30_domain::timestamp::MarketTimestamp;
use vn30_market_data::auth::{AuthMethod, DefaultAuthenticator};
use vn30_market_data::dedup::EventDeduplicator;
use vn30_market_data::health::{HealthMonitor, HealthStatus};
use vn30_market_data::parser::{ExchangeErrorEvent, MarketMessage};
use vn30_market_data::reconnect::{MarketConnectionManager, ReconnectPolicy};
use vn30_market_data::subscription::SubscriptionManager;
use vn30_market_data::symbol_mapper::SymbolMapper;
use vn30_market_data::websocket::WebSocketClient;

fn make_ts(secs: i64) -> MarketTimestamp {
    MarketTimestamp::from_epoch_secs(secs).unwrap()
}

#[test]
fn test_qa_dedup_trades_flag_completely_ignored() {
    let mut dedup = EventDeduplicator::new(100, true);
    assert!(dedup.dedup_trades());

    let trade = Trade::new("HPG".to_string(), 28000.0, 500.0, make_ts(1726000000)).unwrap();
    let event = MarketEvent::Trade(trade);

    let dup1 = dedup.is_duplicate(&event);
    assert!(!dup1);

    let dup2 = dedup.is_duplicate(&event);
    println!("Trade duplicate check result with dedup_trades=true: {}", dup2);
    assert!(!dup2, "CONFIRMED: Trade is never deduplicated even when dedup_trades is true!");
    assert_eq!(dedup.dropped_trades(), 0);
}

#[test]
fn test_qa_dedup_negative_zero_hash_discrepancy() {
    let q1 = Quote::new("HPG".to_string(), 28000.0, 100.0, 0.0, 0.0, make_ts(1726000000)).unwrap();
    let q2 = Quote::new("HPG".to_string(), 28000.0, 100.0, -0.0, -0.0, make_ts(1726000000)).unwrap();

    let fp1 = EventDeduplicator::fingerprint(&q1);
    let fp2 = EventDeduplicator::fingerprint(&q2);
    println!("Fingerprint q1 (0.0): {:x}, q2 (-0.0): {:x}", fp1, fp2);
    assert_ne!(
        fp1, fp2,
        "CONFIRMED: +0.0 and -0.0 produce different fingerprints despite being numerically equal!"
    );

    let mut dedup = EventDeduplicator::new(100, false);
    let e1 = MarketEvent::Quote(q1);
    let e2 = MarketEvent::Quote(q2);

    assert!(!dedup.is_duplicate(&e1));
    let dup = dedup.is_duplicate(&e2);
    println!("e2 is duplicate of e1: {}", dup);
    assert!(!dup, "CONFIRMED: -0.0 causes duplicate quote to NOT be detected!");
}

#[test]
fn test_qa_symbol_mapper_allows_invalid_canonical_alias() {
    let mut mapper = SymbolMapper::new();
    mapper.register_alias("VN30F1M", "NOT_A_VALID_TICKER_123");

    let map_res = mapper.map("VN30F1M");
    println!("Mapper mapping invalid alias result: {:?}", map_res);
    assert!(
        map_res.is_err(),
        "CONFIRMED BUG: register_alias allows registering invalid canonical symbol, causing runtime failure later!"
    );
}

#[test]
fn test_qa_subscription_manager_allows_invalid_symbols() {
    let mut sub = SubscriptionManager::new();
    let res = sub.subscribe(["INVALID_LONG_TICKER_XYZ_123", "!@#$%^&*"]);
    println!("SubscriptionManager invalid symbols result: {:?}", res);
    assert!(
        res.is_ok(),
        "CONFIRMED: SubscriptionManager does not validate symbol schema and accepts garbage symbols!"
    );
    assert!(sub.is_subscribed("!@#$%^&*"));
}

#[test]
fn test_qa_health_monitor_clock_skew_or_out_of_order() {
    let monitor = HealthMonitor::new(5_000, 30_000);
    monitor.record_message(20_000);

    let status = monitor.check_health(15_000);
    println!("Health status during clock skew: {:?}", status);
    assert_eq!(
        status,
        HealthStatus::Healthy,
        "saturating_sub returns 0, so it always reports Healthy even when clock skewed"
    );
}

#[test]
fn test_qa_health_monitor_inverted_timeouts() {
    let monitor = HealthMonitor::new(30_000, 10_000);
    monitor.record_message(10_000);

    let status = monitor.check_health(25_000);
    println!("Health status with inverted timeout: {:?}", status);
    assert_eq!(status, HealthStatus::Stale);
}

#[test]
fn test_qa_exchange_error_misclassified_as_parse_error() {
    let msg = MarketMessage::ExchangeError(ExchangeErrorEvent {
        code: "RATE_LIMIT".to_string(),
        message: "Too many requests to exchange".to_string(),
    });

    let res = msg.try_into_market_event();
    println!("ExchangeError try_into_market_event result: {:?}", res);
    match res {
        Err(MarketDataError::ParseError(msg)) => {
            assert_eq!(msg, "Too many requests to exchange");
            println!("CONFIRMED BUG: ExchangeError is returned as ParseError!");
        }
        other => panic!("Expected ParseError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_qa_reconnect_manager_premature_termination_on_max_retries() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ws_url = format!("ws://{}", addr);

    let server_handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.send(Message::Text("{\"type\":\"heartbeat\",\"timestamp\":1724900000}".into())).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = ws.close(None).await;
        }
    });

    let (tx, _rx) = mpsc::channel(100);
    let sub = Arc::new(tokio::sync::RwLock::new(SubscriptionManager::new()));
    let health = Arc::new(HealthMonitor::new(5000, 30000));
    let auth = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
    let client = WebSocketClient::new(ws_url, 100);

    let mut policy = ReconnectPolicy::new(10, 100, 2.0);
    policy.max_retries = Some(1);

    let mut manager = MarketConnectionManager::new(
        client,
        auth,
        sub,
        health,
        tx,
        policy,
    );

    let manager_task = tokio::spawn(async move {
        manager.run().await;
    });

    let result = tokio::time::timeout(Duration::from_millis(500), manager_task).await;
    println!("Manager task finished: {:?}", result.is_ok());
    assert!(
        result.is_err(),
        "MarketConnectionManager should NOT terminate immediately on first clean disconnect; it must attempt reconnection"
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_qa_server_ping_ignored_and_drops_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ws_url = format!("ws://{}", addr);

    let server_handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.send(Message::Ping(vec![1, 2, 3].into())).await;
            let res = tokio::time::timeout(Duration::from_millis(200), ws.next()).await;
            match res {
                Ok(Some(Ok(Message::Pong(data)))) => {
                    println!("Server received Pong: {:?}", data);
                }
                Ok(other) => {
                    println!("Server did NOT receive Pong, got: {:?}", other);
                }
                Err(_) => {
                    println!("Server timed out waiting for Pong from client!");
                }
            }
        }
    });

    let (tx, _rx) = mpsc::channel(100);
    let sub = Arc::new(tokio::sync::RwLock::new(SubscriptionManager::new()));
    let health = Arc::new(HealthMonitor::new(5000, 30000));
    let auth = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
    let client = WebSocketClient::new(ws_url, 100);
    let policy = ReconnectPolicy::new(10, 100, 2.0);

    let mut manager = MarketConnectionManager::new(
        client,
        auth,
        sub,
        health,
        tx,
        policy,
    );

    let mut ws_read = manager.connect_and_handshake().await.unwrap();
    let msg = ws_read.next().await;
    println!("Client received from server: {:?}", msg);
    assert!(matches!(msg, Some(Ok(Message::Ping(_)))));

    server_handle.await.unwrap();
}
