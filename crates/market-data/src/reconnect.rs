use crate::Authenticator;
use crate::HealthStatus;
use crate::SubscriptionManager;
use crate::WebSocketClient;
use crate::{ConnectionState, HealthMonitor, RawMarketMessage};
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use vn30_domain::errors::MarketDataError;

pub struct MarketConnectionManager {
    pub client: WebSocketClient,
    pub authenticator: Arc<dyn Authenticator>,
    pub current_epoch: AtomicU64,
    pub subscription_manager: Arc<tokio::sync::RwLock<SubscriptionManager>>,
    pub state: Arc<tokio::sync::RwLock<ConnectionState>>,
    pub event_sender: mpsc::Sender<RawMarketMessage>,
    pub reconnect_policy: ReconnectPolicy,
    pub health_monitor: Arc<HealthMonitor>,
}

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub max_retries: Option<usize>,
    pub backoff_factor: f64,
}

impl ReconnectPolicy {
    pub fn new(initial: u64, max: u64, backoff: f64) -> Self {
        Self {
            initial_backoff_ms: initial,
            max_backoff_ms: max,
            max_retries: None,
            backoff_factor: backoff,
        }
    }

    pub fn calculate_delay_ms(&self, attempt: usize) -> u64 {
        // tính thời gian chờ tăng dần theo cấp số nhân và không vượt quá max_backoff_ms).
        let base_delay = (self.initial_backoff_ms as f64 * self.backoff_factor.powf(attempt as f64))
            .round() as u64;

        base_delay.min(self.max_backoff_ms)
    }
}

impl MarketConnectionManager {
    pub fn new(
        client: WebSocketClient,
        authenticator: Arc<dyn Authenticator>,
        subscription_manager: Arc<tokio::sync::RwLock<SubscriptionManager>>,
        health_monitor: Arc<HealthMonitor>,
        event_sender: mpsc::Sender<RawMarketMessage>,
        reconnect_policy: ReconnectPolicy,
    ) -> Self {
        Self {
            client,
            authenticator,
            current_epoch: AtomicU64::new(0),
            subscription_manager,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            event_sender,
            reconnect_policy,
            health_monitor,
        }
    }

    pub async fn connect_and_handshake(
        &mut self,
    ) -> Result<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, MarketDataError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.client.endpoint)
            .await
            .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;

        let (mut ws_write, mut ws_read) = ws_stream.split();

        *self.state.write().await = ConnectionState::Connecting;

        self.current_epoch.fetch_add(1, Ordering::Relaxed);

        if let Some(auth_payload) = self.authenticator.generate_auth_message()? {
            ws_write
                .send(Message::Text(auth_payload.into()))
                .await
                .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;

            if let Some(msg_res) = ws_read.next().await {
                let msg = msg_res.map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;
                if let Message::Text(text) = msg {
                    self.authenticator.verify_auth_response(&text)?;
                }
            } else {
                return Err(MarketDataError::ConnectionError(
                    "Server closed connection during auth handshake".to_string(),
                ));
            }
        }

        if let Some(subscribe_payload) = self
            .subscription_manager
            .read()
            .await
            .generate_resubscribe_message()?
        {
            ws_write
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    subscribe_payload.into(),
                ))
                .await
                .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;
        }

        *self.state.write().await = ConnectionState::Connected;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.health_monitor.record_message(now_ms);

        Ok(ws_read)
    }

    pub async fn run(&mut self) {
        let mut attempt = 0;

        loop {
            if Some(attempt) >= self.reconnect_policy.max_retries {
                tracing::error!("Đã đạt đến số lần kết nối lại tối đa");
                break;
            }

            if attempt > 0 {
                *self.state.write().await = ConnectionState::Reconnecting;
                let delay_ms = self.reconnect_policy.calculate_delay_ms(attempt);
                tracing::info!("Đang kết nối lại sau {}ms", delay_ms);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            *self.state.write().await = ConnectionState::Connecting;

            self.current_epoch.fetch_add(1, Ordering::Relaxed);
            let mut health_interval = tokio::time::interval(std::time::Duration::from_secs(2));

            let connect_future = self.connect_and_handshake();
            match connect_future.await {
                Ok(mut ws_read) => {
                    *self.state.write().await = ConnectionState::Connected;
                    loop {
                        tokio::select! {
                            biased;
                            maybe_msg = ws_read.next() => {
                                match maybe_msg {
                                    Some(Ok(Message::Text(text))) => {
                                        let timestamp = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as u64;
                                        self.health_monitor.record_heartbeat(timestamp);
                                        self.event_sender
                                            .send(RawMarketMessage::Text(text.to_string()))
                                            .await
                                            .unwrap_or_default();
                                    }
                                    Some(Ok(Message::Close(close_frame))) => {
                                        tracing::warn!("WebSocket server closed the connection: {:?}", close_frame);
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("WebSocket read error: {}", e);
                                        break;
                                    }
                                    None => {
                                        tracing::warn!("WebSocket stream closed (None)");
                                        break;
                                    }
                                    _ => ()
                                }
                            }

                            _ = health_interval.tick() =>{
                                let time_stamp = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as u64;
                                let status = self.health_monitor.check_health(time_stamp);

                                match status {
                                    HealthStatus::Stale=> {
                                        tracing::warn!("WS connection is stale, will reconnect");
                                        break;
                                    }
                                    HealthStatus::Dead=> {
                                        tracing::warn!("WS connection is dead, will reconnect");
                                        break;
                                    }
                                    _ => ()
                                }
                            }
                        }
                    }
                    *self.state.write().await = ConnectionState::Disconnected;
                    attempt = 1;
                }
                Err(e) => {
                    tracing::error!("Lỗi khi kết nối: {}", e);
                    attempt += 1;
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthMethod, DefaultAuthenticator};
    use tokio::net::TcpListener;

    #[test]
    fn test_reconnect_policy_calculate_delay_exponential() {
        let policy = ReconnectPolicy::new(1000, 30000, 2.0);

        assert_eq!(policy.calculate_delay_ms(0), 1000);
        assert_eq!(policy.calculate_delay_ms(1), 2000);
        assert_eq!(policy.calculate_delay_ms(2), 4000);
        assert_eq!(policy.calculate_delay_ms(3), 8000);
        assert_eq!(policy.calculate_delay_ms(4), 16000);
    }

    #[test]
    fn test_reconnect_policy_max_backoff_capped() {
        let policy = ReconnectPolicy::new(1000, 10000, 2.0);

        // Với attempt lớn, delay không được vượt quá max_backoff_ms (10000ms)
        assert_eq!(policy.calculate_delay_ms(5), 10000);
        assert_eq!(policy.calculate_delay_ms(10), 10000);
    }

    #[test]
    fn test_reconnect_policy_custom_factor() {
        let policy = ReconnectPolicy::new(500, 5000, 1.5);

        assert_eq!(policy.calculate_delay_ms(0), 500);
        assert_eq!(policy.calculate_delay_ms(1), 750);
        assert_eq!(policy.calculate_delay_ms(2), 1125);
    }

    #[tokio::test]
    async fn test_market_connection_manager_initial_state() {
        let client = WebSocketClient::new("ws://127.0.0.1:8080", 100);
        let authenticator = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
        let subscription_manager = Arc::new(RwLock::new(SubscriptionManager::new()));
        let health_monitor = Arc::new(HealthMonitor::new(5000, 30000));
        let (tx, _rx) = mpsc::channel(100);
        let policy = ReconnectPolicy::new(1000, 30000, 2.0);

        let manager = MarketConnectionManager::new(
            client,
            authenticator,
            subscription_manager,
            health_monitor,
            tx,
            policy,
        );

        assert_eq!(
            *manager.state.read().await,
            ConnectionState::Disconnected
        );
        assert_eq!(manager.current_epoch.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_connect_and_handshake_unreachable_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // Đảm bảo port không mở

        let client = WebSocketClient::new(format!("ws://{}", addr), 100);
        let authenticator = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
        let subscription_manager = Arc::new(RwLock::new(SubscriptionManager::new()));
        let health_monitor = Arc::new(HealthMonitor::new(5000, 30000));
        let (tx, _rx) = mpsc::channel(100);
        let policy = ReconnectPolicy::new(1000, 30000, 2.0);

        let mut manager = MarketConnectionManager::new(
            client,
            authenticator,
            subscription_manager,
            health_monitor,
            tx,
            policy,
        );

        let result = manager.connect_and_handshake().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ConnectionError(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Mong đợi ConnectionError nhưng nhận: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_connect_and_handshake_success_with_mock_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ws_url = format!("ws://{}", addr);

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws_stream
                .send(Message::Text("{\"type\":\"heartbeat\"}".into()))
                .await
                .unwrap();
        });

        let client = WebSocketClient::new(ws_url, 100);
        let authenticator = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
        let subscription_manager = Arc::new(RwLock::new(SubscriptionManager::new()));
        let health_monitor = Arc::new(HealthMonitor::new(5000, 30000));
        let (tx, _rx) = mpsc::channel(100);
        let policy = ReconnectPolicy::new(1000, 30000, 2.0);

        let mut manager = MarketConnectionManager::new(
            client,
            authenticator,
            subscription_manager,
            health_monitor,
            tx,
            policy,
        );

        let result = manager.connect_and_handshake().await;
        assert!(result.is_ok());
        assert_eq!(*manager.state.read().await, ConnectionState::Connected);
        assert_eq!(manager.current_epoch.load(Ordering::Relaxed), 1);

        server_handle.abort();
    }
}
