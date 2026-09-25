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

/// Bộ quản lý kết nối thị trường trung tâm (Market Connection Manager).
///
/// Chịu trách nhiệm:
/// - Điều phối vòng đời kết nối WebSocket và chuyển đổi trạng thái ([`ConnectionState`]).
/// - Thực hiện bắt tay xác thực tài khoản ([`Authenticator`]) và tái đăng ký danh mục ([`SubscriptionManager`]).
/// - Tự động tái kết nối theo chính sách lũy thừa ([`ReconnectPolicy`]) khi mất mạng hoặc dữ liệu bị Stale.
/// - Duy trì kênh gửi tin 2 chiều an toàn (`write_tx`) để gửi lệnh subscribe động hoặc phản hồi Ping/Pong.
pub struct MarketConnectionManager {
    pub client: WebSocketClient,
    pub authenticator: Arc<dyn Authenticator>,
    pub current_epoch: AtomicU64,
    pub subscription_manager: Arc<tokio::sync::RwLock<SubscriptionManager>>,
    pub write_tx: Arc<tokio::sync::RwLock<Option<mpsc::Sender<Message>>>>,
    pub state: Arc<tokio::sync::RwLock<ConnectionState>>,
    pub event_sender: mpsc::Sender<RawMarketMessage>,
    pub reconnect_policy: ReconnectPolicy,
    pub health_monitor: Arc<HealthMonitor>,
}

/// Chính sách tái kết nối sử dụng thuật toán suy giảm lũy thừa (Exponential Backoff).
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    /// Độ trễ thử lại ban đầu tính bằng mili-giây (ví dụ: 1.000ms).
    pub initial_backoff_ms: u64,
    /// Độ trễ thử lại tối đa (trần trễ) tính bằng mili-giây (ví dụ: 30.000ms).
    pub max_backoff_ms: u64,
    /// Số lần thử lại tối đa trước khi dừng (None nghĩa là thử lại vô hạn).
    pub max_retries: Option<usize>,
    /// Hệ số nhân lũy thừa giữa các lần thử (ví dụ: 2.0 hoặc 1.5).
    pub backoff_factor: f64,
}

impl ReconnectPolicy {
    /// Khởi tạo một chính sách Reconnect mới.
    ///
    /// # Tham số:
    /// - `initial`: Độ trễ khởi đầu (ms).
    /// - `max`: Độ trễ tối đa (ms).
    /// - `backoff`: Hệ số nhân mũ.
    ///
    /// # Giá trị trả về:
    /// - `Self`: Chính sách tái kết nối cấu hình sẵn.
    pub fn new(initial: u64, max: u64, backoff: f64) -> Self {
        Self {
            initial_backoff_ms: initial,
            max_backoff_ms: max,
            max_retries: None,
            backoff_factor: backoff,
        }
    }

    /// Tính toán thời gian chờ kết nối lại (mili-giây) dựa trên số lần thử thất bại hiện tại (`attempt`).
    ///
    /// # Công thức:
    /// `delay = min(initial_backoff_ms * backoff_factor ^ attempt, max_backoff_ms)`
    ///
    /// # Tham số:
    /// - `attempt`: Số lần kết nối thất bại liên tiếp (1, 2, 3...).
    ///
    /// # Giá trị trả về:
    /// - `u64`: Khoảng thời gian ngủ (sleep) tính bằng mili-giây trước lần kết nối kế tiếp.
    pub fn calculate_delay_ms(&self, attempt: usize) -> u64 {
        // tính thời gian chờ tăng dần theo cấp số nhân và không vượt quá max_backoff_ms).
        let base_delay = (self.initial_backoff_ms as f64 * self.backoff_factor.powf(attempt as f64))
            .round() as u64;

        base_delay.min(self.max_backoff_ms)
    }
}

impl MarketConnectionManager {
    /// Khởi tạo một `MarketConnectionManager` mới.
    ///
    /// # Tham số:
    /// - `client`: WebSocketClient chứa endpoint kết nối.
    /// - `authenticator`: Bộ sinh/thẩm định gói tin xác thực.
    /// - `subscription_manager`: Quản lý danh sách các mã cổ phiếu đang theo dõi.
    /// - `health_monitor`: Giám sát nhịp tim và dữ liệu quá hạn.
    /// - `event_sender`: Kênh gửi các frame thô thu được tới pipeline phân tích.
    /// - `reconnect_policy`: Quy tắc tính thời gian chờ tái kết nối.
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
            write_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Thiết lập kết nối WebSocket và hoàn tất các thủ tục bắt tay ban đầu (Handshake).
    ///
    /// # Các bước thực hiện:
    /// 1. Kết nối TCP/TLS tới endpoint WebSocket.
    /// 2. Gửi frame xác thực (nếu cấu hình yêu cầu) và chờ thẩm định phản hồi từ sàn.
    /// 3. Gửi frame đăng ký nhận dữ liệu cho toàn bộ các mã đang theo dõi ([`SubscriptionManager::generate_resubscribe_message`]).
    /// 4. Cập nhật trạng thái sang [`ConnectionState::Connected`] và ghi nhận mốc thời gian vào `health_monitor`.
    /// 5. Tạo kênh `write_tx` (dung lượng 1024) và tách riêng writer task để hỗ trợ gửi tin 2 chiều.
    ///
    /// # Giá trị trả về:
    /// - `Ok(SplitStream)`: Luồng đọc (`ws_read`) của WebSocket connection.
    /// - `Err(MarketDataError::ConnectionError)`: Nếu kết nối, xác thực hoặc đăng ký thất bại.
    pub async fn connect_and_handshake(
        &mut self,
    ) -> Result<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, MarketDataError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.client.endpoint)
            .await
            .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;

        let (mut ws_write, mut ws_read) = ws_stream.split();

        let (tx, mut rx) = mpsc::channel::<Message>(1024);

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

        *self.write_tx.write().await = Some(tx.clone());
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    tracing::error!("Failed to send message: {}", e);
                    break;
                }
            }

            let _ = ws_write.close().await;
        });

        Ok(ws_read)
    }

    /// Vòng lặp điều phối chính (Event Loop) quản lý kết nối dài hạn và tự phục hồi (Self-Healing).
    ///
    /// Chạy liên tục để:
    /// - Quản lý máy trạng thái Reconnect kết hợp kiểm tra `reconnect_policy`.
    /// - Đọc các frame WebSocket đến, phản hồi Pong tự động cho các Ping của sàn.
    /// - Giám sát định kỳ 2 giây/lần bằng `health_monitor`, chủ động ngắt kết nối và thử lại nếu phát hiện Stale hoặc Dead.
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
                    attempt = 0;
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
                                        if text.contains("\"type\":\"ping\"") {
                                            let pong = serde_json::json!({"type": "pong"}).to_string();
                                            let _ = self.send_message(Message::Text(pong.into())).await;
                                        }
                                    }
                                    Some(Ok(Message::Ping(payload))) => {
                                        tracing::debug!("Nhận WebSocket Ping từ sàn, gửi Pong phản hồi");
                                        let _ = self.send_message(Message::Pong(payload)).await;
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
                    *self.write_tx.write().await = None;
                }
                Err(e) => {
                    tracing::error!("Lỗi khi kết nối: {}", e);
                    attempt += 1;
                    continue;
                }
            }
        }
    }

    /// Gửi một frame WebSocket bất đồng bộ lên sàn giao dịch thông qua writer channel.
    ///
    /// # Tham số:
    /// - `message`: Tungstenite WebSocket [`Message`] cần gửi (Text, Binary, Ping, Pong...).
    ///
    /// # Giá trị trả về:
    /// - `Ok(())`: Gửi frame vào hàng đợi writer thành công.
    /// - `Err(MarketDataError::ConnectionError)`: Nếu kết nối hiện đang ngắt hoặc writer task đã bị đóng.
    pub async fn send_message(&self, message: Message) -> Result<(), MarketDataError> {
        let guard = self.write_tx.read().await;
        if let Some(ws_write) = guard.as_ref() {
            ws_write
                .send(message)
                .await
                .map_err(|e| MarketDataError::ConnectionError(e.to_string()))
        } else {
            Err(MarketDataError::ConnectionError(
                "No active connection".to_string(),
            ))
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

        assert_eq!(*manager.state.read().await, ConnectionState::Disconnected);
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
