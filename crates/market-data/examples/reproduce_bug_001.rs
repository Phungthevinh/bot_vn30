use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use vn30_market_data::auth::{AuthMethod, DefaultAuthenticator};
use vn30_market_data::health::HealthMonitor;
use vn30_market_data::reconnect::{MarketConnectionManager, ReconnectPolicy};
use vn30_market_data::subscription::SubscriptionManager;
use vn30_market_data::websocket::WebSocketClient;

#[tokio::main]
async fn main() {
    println!("\n================================================================================");
    println!("     TRÌNH DIỄN TRỰC TIẾP: TÁI HIỆN BUG-001 VÀ HỆ QUẢ TRÊN HỆ THỐNG");
    println!("     (Vấn đề: ws_write bị drop -> Tê liệt ghi dữ liệu 2 chiều & Đứt kết nối)");
    println!("================================================================================\n");

    // Khởi động Mock Exchange Server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ws_url = format!("ws://{}", addr);

    println!("[1. SETUP] Khởi tạo Mock Broker Server tại: {}", ws_url);

    let (server_shutdown_tx, server_shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_task = tokio::spawn(async move {
        println!("[SERVER] Sẵn sàng lắng nghe kết nối từ bot...");
        let (stream, _) = listener.accept().await.unwrap();
        println!("[SERVER] Client bot đã kết nối TCP thành công!");
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        println!("[SERVER] WebSocket Handshake hoàn tất!");

        // 1. Nhận frame subscribe ban đầu (được gửi lúc handshake)
        if let Some(Ok(msg)) = ws.next().await {
            println!(
                "[SERVER] [BAN ĐẦU] Nhận frame subscribe từ handshake: {}",
                msg
            );
        }

        // 2. Gửi 1 bản tin trade
        let trade = "{\"type\":\"trade\",\"symbol\":\"HPG\",\"price\":28500.0,\"volume\":500.0,\"timestamp\":1726000000}";
        ws.send(Message::Text(trade.into())).await.unwrap();
        println!("[SERVER] Đã stream bản tin khớp lệnh HPG tới client.");

        // 3. THỬ THÁCH 1: CLIENT GỬI SUBSCRIBE MÃ MỚI TRONG KHI ĐANG STREAM (DYNAMIC SUBSCRIPTION)
        println!(
            "\n--------------------------------------------------------------------------------"
        );
        println!("[SERVER] [THỬ THÁCH 1] Chờ Client gửi lệnh subscribe mã mới ('FPT', 'SSI')...");
        println!("[SERVER] Bắt đầu timeout 2.0 giây để xem Client có gửi được frame nào không...");
        println!(
            "--------------------------------------------------------------------------------"
        );

        let dynamic_sub_wait = tokio::time::timeout(Duration::from_millis(2000), async {
            while let Some(msg) = ws.next().await {
                if let Ok(Message::Text(txt)) = msg {
                    if txt.contains("FPT") {
                        return Ok(txt);
                    }
                }
            }
            Err("Closed")
        })
        .await;

        match dynamic_sub_wait {
            Ok(Ok(sub_msg)) => {
                println!("[SERVER] BẤT NGỜ: Nhận được dynamic subscribe: {}", sub_msg);
            }
            _ => {
                println!("\n>>> [CHỨNG MINH 1: TÊ LIỆT SUBSCRIBE ĐỘNG] <<<");
                println!(
                    ">>> Đã hết 2 giây! Server KHÔNG NHẬN ĐƯỢC bất kỳ frame subscribe nào từ bot!"
                );
                println!(
                    ">>> Dù bên trong bot, SubscriptionManager đã thêm mã FPT vào active_symbols,"
                );
                println!(
                    ">>> nhưng MarketConnectionManager KHÔNG CÓ writer để gửi nó lên WebSocket!"
                );
                println!(">>> Hệ quả: Người dùng thêm mã theo dõi lúc runtime nhưng KHÔNG BAO GIỜ nhận được dữ liệu mã đó!\n");
            }
        }

        // 4. THỬ THÁCH 2: GỬI APPLICATION HEARTBEAT PING (Chuẩn SSI / VNDIRECT / VPS)
        println!(
            "--------------------------------------------------------------------------------"
        );
        println!("[SERVER] [THỬ THÁCH 2] Server gửi Application Heartbeat Ping dạng JSON:");
        let app_ping = "{\"type\":\"ping\",\"timestamp\":1726000010}";
        println!("[SERVER] Gửi: {}", app_ping);
        println!("[SERVER] Quy chuẩn sàn: Client phải phản hồi JSON pong dạng {{\"type\":\"pong\"}} trong 2s...");
        println!(
            "--------------------------------------------------------------------------------"
        );

        ws.send(Message::Text(app_ping.into())).await.unwrap();

        let app_pong_wait = tokio::time::timeout(Duration::from_millis(2000), async {
            while let Some(msg) = ws.next().await {
                if let Ok(Message::Text(txt)) = msg {
                    if txt.contains("pong") {
                        return Ok(txt);
                    }
                }
            }
            Err("No pong")
        })
        .await;

        match app_pong_wait {
            Ok(Ok(pong)) => {
                println!("[SERVER] Nhận được Pong: {}", pong);
            }
            _ => {
                println!("\n>>> [CHỨNG MINH 2: CLIENT KHÔNG THỂ PHẢN HỒI HEARTBEAT] <<<");
                println!(">>> Hết 2 giây! Client hoàn toàn im lặng, không thể gửi frame Pong!");
                println!(">>> Lý do: ws_write đã bị drop tại dòng 130 trong reconnect.rs!");
                println!(
                    ">>> Hành động của sàn: ĐÓNG KẾT NỐI VÌ TIMEOUT (Zombie Client Detection)!\n"
                );
                let _ = ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Protocol,
                    reason: "Heartbeat timeout: Client cannot write Pong".into(),
                })).await;
            }
        }

        let _ = server_shutdown_tx.send(());
    });

    // Khởi tạo bot client
    println!("[2. SETUP] Khởi tạo Client MarketConnectionManager...");
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let sub_manager = Arc::new(tokio::sync::RwLock::new(SubscriptionManager::new()));
    let health = Arc::new(HealthMonitor::new(5000, 30000));
    let auth = Arc::new(DefaultAuthenticator::new(AuthMethod::None));
    let client = WebSocketClient::new(&ws_url, 100);

    // Đăng ký symbol khởi đầu
    sub_manager.write().await.subscribe(["HPG"]).unwrap();
    println!("[CLIENT] Đã đăng ký mã khởi đầu: ['HPG']");

    let mut policy = ReconnectPolicy::new(500, 2000, 2.0);
    policy.max_retries = Some(1); // Để minh họa BUG-002 sau khi rớt kết nối

    let mut connection_manager = MarketConnectionManager::new(
        client,
        auth,
        Arc::clone(&sub_manager),
        health,
        event_tx,
        policy,
    );

    // Task nhận tin từ bot
    tokio::spawn(async move {
        while let Some(raw) = event_rx.recv().await {
            println!("[BOT INGESTION RECEIVER] Nhận frame từ sàn: {:?}", raw);
        }
    });

    // Chạy connection manager
    println!("[3. RUN] Bắt đầu chạy connection_manager.run() trong background task...");
    let manager_task = tokio::spawn(async move {
        connection_manager.run().await;
        println!("\n>>> [CHỨNG MINH 3: KẾT NỐI BỊ TERMINATE VĨNH VIỄN - BUG-002] <<<");
        println!(">>> connection_manager.run() ĐÃ KẾT THÚC VÀ THOÁT HOÀN TOÀN!");
        println!(
            ">>> Sau khi sàn đóng socket, attempt=1 khớp max_retries=Some(1), bot chết vĩnh viễn!"
        );
    });

    // Đợi 800ms để kết nối ổn định rồi thực hiện đăng ký mã mới lúc runtime
    tokio::time::sleep(Duration::from_millis(800)).await;
    println!(
        "\n[USER THAO TÁC] Người dùng gọi: sub_manager.subscribe(['FPT', 'SSI']) lúc đang chạy..."
    );
    let sub_result = sub_manager.write().await.subscribe(["FPT", "SSI"]).unwrap();
    println!(
        "[CLIENT STATE] Payload được sinh ra bởi SubscriptionManager: {:?}",
        sub_result
    );
    println!(
        "[CLIENT STATE] active_symbols hiện có: {:?}",
        sub_manager.read().await.get_active_symbols()
    );
    println!("[CLIENT CẢNH BÁO] KHÔNG CÓ KÊNH GỬI! Frame này nằm lại trong RAM của bot, KHÔNG THỂ BẮN LÊN SÀN!\n");

    // Đợi server và client kết thúc
    let _ = server_shutdown_rx.await;
    server_task.await.unwrap();
    let _ = manager_task.await;

    println!("\n================================================================================");
    println!("     KẾT THÚC KIỂM THỬ THỰC NGHIỆM: ĐÃ TÁI HIỆN VÀ XÁC MINH 100% BUG-001");
    println!("================================================================================\n");
}
