//! WebSocket Client implementation for Market Data Ingestion.
//!
//! Handles TCP/TLS connection, framed message loop, and bounded mpsc stream.

use futures_util::StreamExt;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use vn30_domain::errors::MarketDataError;

/// Trạng thái vòng đời kết nối WebSocket của Client với máy chủ sàn giao dịch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Đang ngắt kết nối (chưa kết nối hoặc đã bị rớt mạng).
    Disconnected,
    /// Đang trong tiến trình thiết lập kết nối TCP/TLS hoặc gửi frame xác thực.
    Connecting,
    /// Kết nối thành công, bắt tay hoàn tất và đang nhận dữ liệu bình thường.
    Connected,
    /// Đang chờ theo chính sách backoff để kết nối lại sau sự cố mất mạng hoặc Stale.
    Reconnecting,
    /// Kết nối đã bị đóng chủ động (Shutdown hoặc chấm dứt tiến trình).
    Closed,
}

/// Định dạng bản tin thô nhận được từ socket stream trước khi phân tích cú pháp.
#[derive(Debug, Clone, PartialEq)]
pub enum RawMarketMessage {
    /// Frame dữ liệu văn bản UTF-8 (chuỗi JSON payload).
    Text(String),
    /// Frame dữ liệu nhị phân (Binary payload).
    Binary(Vec<u8>),
    /// Frame Ping kiểm tra kết nối từ máy chủ WebSocket.
    Ping(Vec<u8>),
    /// Frame Pong phản hồi tín hiệu Ping.
    Pong(Vec<u8>),
}

/// Client WebSocket chịu trách nhiệm kết nối mạng TCP/TLS tới sàn giao dịch.
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    /// Địa chỉ WebSocket endpoint (ví dụ: `"wss://api.provider.vn/stream"`).
    pub endpoint: String,
    /// Kích thước bộ đệm kênh MPSC bounded channel chống tràn bộ nhớ.
    pub channel_capacity: usize,
}

impl WebSocketClient {
    /// Khởi tạo một thực thể [`WebSocketClient`] mới với endpoint và dung lượng buffer tương ứng.
    ///
    /// # Tham số:
    /// - `endpoint`: Địa chỉ URL WebSocket của nhà cung cấp (ví dụ: `"wss://api.ssi.com.vn/stream"`).
    /// - `channel_capacity`: Kích thước tối đa của kênh mpsc bounded channel dùng để đệm các frame thô nhận được.
    ///
    /// # Giá trị trả về:
    /// - `Self`: Client sẵn sàng kết nối.
    pub fn new<S>(endpoint: S, channel_capacity: usize) -> Self
    where
        S: Into<String>,
    {
        Self {
            endpoint: endpoint.into(),
            channel_capacity,
        }
    }

    /// Thiết lập kết nối bất đồng bộ tới máy chủ WebSocket và kích hoạt vòng lặp nhận frame ngầm định.
    ///
    /// # Cơ chế:
    /// 1. Kết nối TCP/TLS tới `endpoint` thông qua `tokio-tungstenite`.
    /// 2. Khởi tạo một bounded mpsc channel với dung lượng `channel_capacity` để chống tràn bộ nhớ (backpressure).
    /// 3. Khởi chạy một tokio task chạy ngầm liên tục lắng nghe frame từ socket, phân loại thành [`RawMarketMessage`]
    ///    và chuyển tiếp vào channel.
    ///
    /// # Giá trị trả về:
    /// - `Ok((Receiver<RawMarketMessage>, JoinHandle<()>))`:
    ///   - Đầu đọc channel (`Receiver`) để tầng trên tiêu thụ dữ liệu.
    ///   - `JoinHandle` của background reader task để giám sát vòng đời.
    ///
    /// # Lỗi trả về:
    /// - [`MarketDataError::ConnectionError`]: Nếu bắt tay TCP hoặc TLS thất bại, hoặc URL endpoint không hợp lệ.
    pub async fn connect(
        &self,
    ) -> Result<(Receiver<RawMarketMessage>, JoinHandle<()>), MarketDataError> {
        let (mut ws_stream, _) = tokio_tungstenite::connect_async(&self.endpoint)
            .await
            .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;

        // 1. Tạo bounded mpsc channel với self.channel_capacity
        let (tx, rx) = tokio::sync::mpsc::channel::<RawMarketMessage>(self.channel_capacity);

        // 3. Spawn background tokio task để đọc frame từ ws_stream và gửi vào tx channel
        let read_handle = tokio::spawn(async move {
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if tx
                            .send(RawMarketMessage::Text(text.to_string()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Ok(Message::Binary(bin)) => {
                        if tx
                            .send(RawMarketMessage::Binary(bin.to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Ok(Message::Ping(ping_data)) => {
                        if tx
                            .send(RawMarketMessage::Ping(ping_data.to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Ok(Message::Pong(pong_data)) => {
                        if tx
                            .send(RawMarketMessage::Pong(pong_data.to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    Ok(Message::Close(_)) => {
                        tracing::info!("WebSocket server đã đóng kết nối (Close frame)");
                        break;
                    }

                    Ok(Message::Frame(_)) => {
                        // Bỏ qua raw frame (thường không cần xử lý)
                    }

                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        // 4. Trả về (rx, handle)
        Ok((rx, read_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::SinkExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_client_initialization() {
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

        let client = WebSocketClient::new(ws_url, 10);
        let (mut rx, handle) = client.connect().await.unwrap();
        let received = rx.recv().await;
        assert_eq!(
            received,
            Some(RawMarketMessage::Text(
                "{\"type\":\"heartbeat\"}".to_string()
            ))
        );

        handle.abort();
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_client_connection_error_on_unreachable_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // Ensure port is not listening

        let ws_url = format!("ws://{}", addr);
        let client = WebSocketClient::new(ws_url, 10);
        let result = client.connect().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ConnectionError(err) => {
                assert!(!err.is_empty());
            }
            other => panic!("Expected ConnectionError, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_client_receives_multiple_frame_types() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ws_url = format!("ws://{}", addr);

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws_stream
                .send(Message::Binary(vec![1, 2, 3].into()))
                .await
                .unwrap();
            ws_stream
                .send(Message::Ping(vec![4, 5].into()))
                .await
                .unwrap();
            ws_stream
                .send(Message::Pong(vec![6, 7].into()))
                .await
                .unwrap();
        });

        let client = WebSocketClient::new(ws_url, 10);
        let (mut rx, handle) = client.connect().await.unwrap();

        assert_eq!(
            rx.recv().await,
            Some(RawMarketMessage::Binary(vec![1, 2, 3]))
        );
        assert_eq!(rx.recv().await, Some(RawMarketMessage::Ping(vec![4, 5])));
        assert_eq!(rx.recv().await, Some(RawMarketMessage::Pong(vec![6, 7])));

        handle.abort();
        server_handle.abort();
    }
}
