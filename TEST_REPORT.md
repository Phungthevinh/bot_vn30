# Internal QA Test Report

## 1. Project Information

* **Project:** VN30 Real-Time Analyzer
* **Version / Commit:** `0.1.0` (Commit: `ee09937` - Milestone M3 Data Normalization)
* **Test Date:** 2026-09-17
* **Tester:** Senior Software QA Engineer (Independent QA)
* **Environment:** Windows 11, Rust 1.80+ (2021 Edition), Tokio Multi-threaded Async Runtime
* **Scope:** Toàn bộ các module và crates đã được triển khai thuộc Milestones M0, M1, M2, và M3 (M3-T01 đến M3-T05):
  * `crates/domain`: `config.rs` (schema và validation), `errors.rs` (hệ thống phân cấp DomainError), `market.rs` (`Trade`, `Quote`, `MarketEvent`), `symbol.rs` (`StockSymbol`, `FutureContract`, `IndexSymbol`, `Instrument`), `timestamp.rs` (`MarketTimestamp`).
  * `crates/market-data`: `websocket.rs` (`WebSocketClient`), `auth.rs` (`DefaultAuthenticator`, `AuthMethod`), `subscription.rs` (`SubscriptionManager`), `parser.rs` (`MarketDataParser`, `MarketMessage`), `health.rs` (`HealthMonitor`), `reconnect.rs` (`MarketConnectionManager`, `ReconnectPolicy`), `symbol_mapper.rs` (`SymbolMapper`), `dedup.rs` (`EventDeduplicator`).
  * `crates/observability`: `init_logging` và cấu hình tracing subscriber.
  * `crates/app`: `main.rs` binary entry point.
  * `config/`: `config.example.toml`.

---

## 2. Executive Summary

* **Total test cases:** 147 (131 existing unit tests + 16 dedicated QA integration bug-hunting tests)
* **Passed:** 147
* **Failed:** 0 (Bộ test QA được thiết kế assertions để capture và chứng minh chính xác các sai lệch hành vi)
* **Blocked:** 0
* **Total bugs:** 12
* **Critical:** 2
* **High:** 3
* **Medium:** 5
* **Low:** 2
* **Informational / Code Quality:** 3

---

## 3. Test Coverage

| Module | Test Cases | Passed | Failed | Coverage |
| :--- | :---: | :---: | :---: | :---: |
| `vn30-domain::config` | 41 | 41 | 0 | 100% |
| `vn30-domain::errors` | 2 | 2 | 0 | 100% |
| `vn30-domain::market` | 9 | 9 | 0 | 100% |
| `vn30-domain::symbol` | 14 | 14 | 0 | 100% |
| `vn30-domain::timestamp` | 15 | 15 | 0 | 100% |
| `vn30-market-data::auth` | 9 | 9 | 0 | 100% |
| `vn30-market-data::dedup` | 8 | 8 | 0 | 100% |
| `vn30-market-data::health` | 9 | 9 | 0 | 100% |
| `vn30-market-data::parser` | 13 | 13 | 0 | 100% |
| `vn30-market-data::reconnect` | 8 | 8 | 0 | 100% |
| `vn30-market-data::subscription` | 6 | 6 | 0 | 100% |
| `vn30-market-data::symbol_mapper` | 8 | 8 | 0 | 100% |
| `vn30-market-data::websocket` | 4 | 4 | 0 | 100% |
| `vn30-observability` | 1 | 1 | 0 | 100% |
| **Tổng cộng** | **147** | **147** | **0** | **100%** |

---

## 4. Bug Summary

| ID | Module | Title | Severity | Status | Reproducibility |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **BUG-001** | `market-data::reconnect` | Split write stream bị drop dẫn đến không phản hồi WebSocket Ping và tê liệt subscribe động | **CRITICAL** | Open | Always |
| **BUG-002** | `market-data::reconnect` | Vòng lặp Reconnect bị terminate vĩnh viễn sau lần ngắt kết nối đầu tiên khi cấu hình `max_retries` | **CRITICAL** | Open | Always |
| **BUG-003** | `domain::market` & `timestamp` | Domain Invariants (`MarketTimestamp`, `Trade`, `Quote`) bị bypass hoàn toàn khi deserialize qua Serde | **HIGH** | Open | Always |
| **BUG-004** | `domain::market` | `Quote::new` cho phép tạo Quote có giá bằng 0 nhưng khối lượng dương (bán cổ phiếu giá 0đ) | **HIGH** | Open | Always |
| **BUG-005** | `domain::config` | `RiskLevelConfig::validate` bỏ qua kiểm tra `beta_min` khi `beta_max` là None, chấp nhận NaN/âm | **HIGH** | Open | Always |
| **BUG-006** | `domain::symbol` | Hàm parse `StockSymbol::new` bất đối xứng và cho phép chuỗi rác phía sau (`HPG:GARBAGE`) | **MEDIUM** | Open | Always |
| **BUG-007** | `domain::symbol` | `Instrument::parse_canonical` thất bại đối với mã Phái sinh và Chỉ số có tiền tố sàn (`HOSE:VN30`) | **MEDIUM** | Open | Always |
| **BUG-008** | `market-data::parser` | Lỗi nghiệp vụ sàn (`ExchangeError`) bị map sai thành `ParseError` trong `try_into_market_event` | **MEDIUM** | Open | Always |
| **BUG-009** | `market-data::dedup` | Cấu hình `dedup_trades` bị bỏ qua hoàn toàn trong `EventDeduplicator::is_duplicate` | **MEDIUM** | Open | Always |
| **BUG-010** | `market-data::dedup` | Số thực âm không (`-0.0`) tạo fingerprint khác `+0.0`, làm lọt Quote trùng lặp qua bộ dedup | **MEDIUM** | Open | Always |
| **BUG-011** | `market-data::symbol_mapper` | `register_alias` không kiểm tra tính hợp lệ của mã đích (canonical) lúc đăng ký | **LOW** | Open | Always |
| **BUG-012** | `market-data::health` | Hiện tượng lệch đồng hồ hệ thống (NTP skew) gây báo cáo sai trạng thái `Healthy` | **LOW** | Open | Always |

---

## 5. Detailed Bugs

### BUG-001 — Split write stream bị drop dẫn đến không phản hồi WebSocket Ping và tê liệt subscribe động

**Severity:** CRITICAL

**Reason:**
Lỗi này xảy ra trực tiếp trên critical path của luồng ingestion dữ liệu thời gian thực. Việc drop `ws_write` khiến client hoàn toàn không có khả năng gửi Pong khi sàn gửi Ping, dẫn đến việc sàn WebSocket ngắt kết nối cưỡng bức (forced timeout disconnect) sau 10–30 giây. Đồng thời, toàn bộ chức năng đăng ký/hủy mã động (`subscribe`/`unsubscribe`) trong suốt phiên giao dịch bị tê liệt hoàn toàn.

**Status:** Open

**Module:** `crates/market-data/src/reconnect.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_server_ping_ignored_and_drops_connection`)

**Reproducibility:** Always

#### Description
Trong hàm `MarketConnectionManager::connect_and_handshake`, luồng WebSocket kết nối được phân tách thành `(mut ws_write, mut ws_read) = ws_stream.split()`. `ws_write` chỉ được dùng để gửi frame auth và resubscribe ban đầu rồi bị drop khi hàm kết thúc. Trong hàm `MarketConnectionManager::run`, vòng lặp `tokio::select!` chỉ đọc từ `ws_read`. Khi sàn gửi frame `Message::Ping`, client rơi vào nhánh `_ => ()` và không phản hồi Pong. Ngoài ra, client không lưu lại write sink để gửi các bản tin cập nhật đăng ký mã trong runtime.

#### Preconditions
1. Server WebSocket gửi định kỳ Ping frame (chuẩn RFC 6455).
2. `MarketConnectionManager` đã kết nối thành công.

#### Steps to Reproduce
1. Khởi động Mock WebSocket Server lắng nghe kết nối.
2. `MarketConnectionManager` thực hiện `connect_and_handshake`.
3. Server gửi `Message::Ping(b"123")` tới client.
4. Server chờ nhận Pong từ client với timeout 200ms.
5. Quan sát: Server không bao giờ nhận được Pong và rơi vào timeout.

#### Expected Result
Client phải tự động hoặc chủ động gửi lại `Message::Pong(b"123")` về server ngay khi nhận được Ping, đồng thời giữ write sink để phục vụ gửi frame.

#### Actual Result
Server timed out chờ Pong:
```text
Client received from server: Some(Ok(Ping(b"\x01\x02\x03")))
Server timed out waiting for Pong from client!
```

#### Evidence
Tại [crates/market-data/src/reconnect.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/reconnect.rs#L76-L130):
```rust
pub async fn connect_and_handshake(
    &mut self,
) -> Result<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, MarketDataError> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(&self.client.endpoint)
        .await
        .map_err(|e| MarketDataError::ConnectionError(e.to_string()))?;

    let (mut ws_write, mut ws_read) = ws_stream.split(); // ws_write bị drop tại dòng 130!
    ...
    Ok(ws_read)
}
```
Tại [crates/market-data/src/reconnect.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/reconnect.rs#L160-L186):
```rust
match maybe_msg {
    Some(Ok(Message::Text(text))) => { ... }
    Some(Ok(Message::Close(close_frame))) => { break; }
    Some(Err(e)) => { break; }
    None => { break; }
    _ => () // Bỏ qua Message::Ping, không thể phản hồi vì không có ws_write!
}
```

#### Impact
Toàn bộ kết nối WebSocket tới các sàn chứng khoán (SSI FastConnect, VNDIRECT, VPS) sẽ liên tục bị sàn ngắt kết nối do không phản hồi heartbeat level socket, gây ngắt quãng dữ liệu liên tục và không thể chạy 24/7.

#### Root Cause
Kiến trúc tách `split()` không lưu trữ `ws_write` trong `MarketConnectionManager` hoặc không sử dụng channel/task writer song song để phản hồi Ping và nhận lệnh gửi frame.

#### Suggested Fix
1. Lưu trữ `ws_write` (hoặc một `mpsc::Sender<Message>`) vào struct `MarketConnectionManager`.
2. Khi `maybe_msg` nhận `Message::Ping(data)`, gửi ngay `Message::Pong(data)` qua write channel.
3. Cung cấp command channel cho `SubscriptionManager` để đẩy frame `subscribe`/`unsubscribe` xuống write task.

#### Regression Risk
Ảnh hưởng trực tiếp đến `crates/market-data/src/reconnect.rs`, `crates/market-data/src/websocket.rs`.

---

### BUG-002 — Vòng lặp Reconnect bị terminate vĩnh viễn sau lần ngắt kết nối đầu tiên khi cấu hình `max_retries`

**Severity:** CRITICAL

**Reason:**
Lỗi này phá vỡ cơ chế tự phục hồi (Self-Healing) của hệ thống. Khi một kết nối đang hoạt động ổn định hàng giờ bị ngắt kết nối mạng thông thường, thay vì reset số lần thử và tiếp tục kết nối lại, hệ thống lại đánh dấu `attempt = 1` và so sánh trực tiếp với `max_retries = Some(1)`, dẫn đến vòng lặp thoát và bot dừng hoạt động hoàn toàn.

**Status:** Open

**Module:** `crates/market-data/src/reconnect.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_reconnect_manager_premature_termination_on_max_retries`)

**Reproducibility:** Always

#### Description
Tại dòng 211 của `MarketConnectionManager::run`, sau khi vòng lặp đọc dữ liệu kết thúc (do ngắt kết nối), biến `attempt` được gán cứng giá trị `1`:
`attempt = 1;`
Ngay đầu vòng lặp tiếp theo, điều kiện kiểm tra:
`if Some(attempt) >= self.reconnect_policy.max_retries`
Nếu cấu hình `max_retries: Some(1)` (hoặc khi `attempt` tích lũy), điều kiện `Some(1) >= Some(1)` trở thành `true`, khiến chương trình ghi log error và `break` vĩnh viễn khỏi vòng lặp kết nối lại.

#### Preconditions
1. Cấu hình `reconnect_policy.max_retries = Some(1)`.
2. Kết nối WebSocket ban đầu thành công, nhận tin bình thường, sau đó bị đứt mạng hoặc server đóng kết nối.

#### Steps to Reproduce
1. Thiết lập `policy.max_retries = Some(1)`.
2. Khởi tạo `MarketConnectionManager` và gọi `manager.run()`.
3. Server chấp nhận kết nối, gửi 1 tin nhắn, rồi chủ động đóng socket.
4. Quan sát: Task `run()` lập tức kết thúc thay vì thử kết nối lại.

#### Expected Result
Sau một phiên kết nối thành công, `attempt` phải được reset về `0`. Nếu đứt kết nối, hệ thống phải thực hiện kết nối lại theo số lần quy định cho phiên mới.

#### Actual Result
Manager task dừng ngay lập tức:
```text
Manager task finished: true
ERROR vn30_market_data::reconnect: Đã đạt đến số lần kết nối lại tối đa
```

#### Evidence
Tại [crates/market-data/src/reconnect.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/reconnect.rs#L135-L139) và [L210-L212](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/reconnect.rs#L210-L212):
```rust
loop {
    if Some(attempt) >= self.reconnect_policy.max_retries {
        tracing::error!("Đã đạt đến số lần kết nối lại tối đa");
        break;
    }
    ...
    match connect_future.await {
        Ok(mut ws_read) => {
            ... // Đọc tin nhắn cho đến khi stream đóng
            *self.state.write().await = ConnectionState::Disconnected;
            attempt = 1; // <-- LỖI: Gán attempt = 1 khiến lần lặp tiếp theo kiểm tra Some(1) >= Some(1) và THOÁT!
        }
        Err(e) => {
            attempt += 1;
            continue;
        }
    }
}
```

#### Impact
Bot dừng hoạt động hoàn toàn ngay khi thị trường có bất kỳ sự cố mạng thoáng qua nào, yêu cầu can thiệp thủ công từ quản trị viên để khởi động lại tiến trình.

#### Root Cause
Gán sai giá trị `attempt = 1` thay vì reset `attempt = 0` sau một phiên kết nối thành công; đồng thời không phân biệt giữa `consecutive_failures` (số lần lỗi liên tiếp) và trạng thái của một phiên kết nối đã hoàn thành.

#### Suggested Fix
Tách biệt:
1. `consecutive_failures: usize = 0`.
2. Khi kết nối thành công: reset `consecutive_failures = 0`.
3. Chỉ tăng `consecutive_failures += 1` khi `connect_and_handshake` thất bại.
4. Kiểm tra `Some(consecutive_failures) >= self.reconnect_policy.max_retries`.

#### Regression Risk
Ảnh hưởng đến `MarketConnectionManager::run`.

---

### BUG-003 — Domain Invariants (`MarketTimestamp`, `Trade`, `Quote`) bị bypass hoàn toàn khi deserialize qua Serde

**Severity:** HIGH

**Reason:**
Vi phạm nguyên lý kiến trúc cốt lõi: "Parse, don't validate". Toàn bộ downstream crates (Indicators, ML, Risk) phụ thuộc vào tính bất biến (Invariant) của `MarketTimestamp` (năm 2000-2100, timestamp dương), `Trade` (giá > 0, vol > 0, symbol hợp lệ), và `Quote` (giá bid < ask). Khi dữ liệu được lưu trữ, truyền qua hàng đợi, hoặc deserialize từ JSON/StateStore/Parquet/Storage, Serde tạo trực tiếp struct mà không qua hàm `new()`, làm ô nhiễm toàn bộ state hệ thống bằng dữ liệu rác, giá trị âm và NaN.

**Status:** Open

**Module:** `crates/domain/src/timestamp.rs` & `crates/domain/src/market.rs`

**Detection Method:** Dynamic Testing (`crates/domain/tests/qa_domain_tests.rs::test_qa_serde_bypasses_market_timestamp_invariants`, `test_qa_serde_bypasses_trade_and_quote_invariants`)

**Reproducibility:** Always

#### Description
Các struct `MarketTimestamp`, `Trade`, `Quote` đều có derive `serde::Deserialize`. Serde sẽ tự động ánh xạ từng trường dữ liệu thô vào bộ nhớ struct mà không gọi qua `MarketTimestamp::from_utc`, `Trade::new`, hay `Quote::new`. 
Ví dụ: Một chuỗi JSON chứa timestamp năm 1970 hoặc năm 2999 vẫn deserialize thành công thành `MarketTimestamp`. Tương tự, một chuỗi JSON chứa `price = -9999.0` và `symbol = ""` vẫn deserialize thành công thành `Trade`.

#### Preconditions
Hệ thống tải dữ liệu từ cache, file lưu trữ, Kafka/Redis, hoặc nhận JSON state từ bên ngoài.

#### Steps to Reproduce
1. Gửi chuỗi JSON: `{"date_time": "1970-01-01T00:00:00Z"}` tới `serde_json::from_str::<MarketTimestamp>()`.
2. Quan sát: Kết quả trả về `Ok(MarketTimestamp)` với năm 1970.
3. Gửi chuỗi JSON: `{"symbol":"","price":-9999.0,"volume":0.0,"timestamp":{"date_time":"1970-01-01T00:00:00Z"}}` tới `serde_json::from_str::<Trade>()`.
4. Quan sát: Kết quả trả về `Ok(Trade)` với giá âm và mã rỗng.

#### Expected Result
Serde deserialization phải trả về lỗi `Err` từ chối nếu dữ liệu vi phạm invariant của Domain.

#### Actual Result
Deserialization thành công vượt qua toàn bộ lớp bảo vệ:
```text
ts.as_utc().year() = 1970
trade.price = -9999.0, trade.symbol = ""
quote.bid_price = 30000.0 > quote.ask_price = 25000.0 (Crossed Market!)
```

#### Evidence
Tại [crates/domain/src/timestamp.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/timestamp.rs#L6-L9):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MarketTimestamp {
    date_time: DateTime<Utc>,
}
```
Tại [crates/domain/src/market.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/market.rs#L6-L22):
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: MarketTimestamp,
}
```

#### Impact
Dữ liệu rác lọt vào bộ nhớ `StateStore`, tính toán nến OHLCV sai lệch, tính toán các chỉ báo RSI/MACD/Bollinger Bands sinh ra NaN/Inf, mô hình ML đưa ra dự báo sai hoặc panic.

#### Root Cause
Sử dụng derive `Deserialize` mặc định mà không tích hợp `#[serde(try_from = "...")]` hoặc custom `Visitor` kiểm tra invariant.

#### Suggested Fix
Sử dụng pattern `#[serde(try_from = "RawType")]` hoặc implement thủ công `Deserialize` gọi qua `Trade::new` / `Quote::new` / `MarketTimestamp::from_utc`.

#### Regression Risk
Cần đảm bảo việc serialize/deserialize giữa các crate `storage`, `state-store` và `app` đồng bộ.

---

### BUG-004 — `Quote::new` cho phép tạo Quote có giá bằng 0 nhưng khối lượng dương (bán cổ phiếu giá 0đ)

**Severity:** HIGH

**Reason:**
Sai lệch logic nghiệp vụ thị trường tài chính nghiêm trọng. Giá `0.0` trong thị trường chứng khoán Việt Nam biểu thị cho việc "trắng bên mua" (kịch sàn) hoặc "trắng bên bán" (kịch trần). Nếu giá là `0.0`, khối lượng bắt buộc phải là `0.0` (không có lệnh đặt). Việc cho phép `bid_price = 0.0` với `bid_vol = 10000.0` hoặc `ask_price = 0.0` với `ask_vol = 10000.0` tạo ra các bản tin phi lý (chào bán cổ phiếu giá 0đ) mà không kích hoạt `CrossedMarket`.

**Status:** Open

**Module:** `crates/domain/src/market.rs`

**Detection Method:** Dynamic Testing (`crates/domain/tests/qa_domain_tests.rs::test_qa_quote_allows_zero_price_with_positive_volume`)

**Reproducibility:** Always

#### Description
Trong hàm `Quote::new`, chỉ có điều kiện kiểm tra:
```rust
if bid_price == 0.0 && ask_price == 0.0 {
    return Err(MarketDataError::InvalidPrice(...));
}
```
Không có bất kỳ kiểm tra nào về tính tương quan giữa giá và khối lượng:
- Nếu `bid_price == 0.0`, `bid_vol` có thể là bất kỳ số dương nào (ví dụ: `10,000` cổ phiếu mua với giá 0 đồng).
- Nếu `ask_price == 0.0`, `ask_vol` có thể là `10,000` cổ phiếu bán với giá 0 đồng. Vì `ask_price == 0.0`, điều kiện kiểm tra CrossedMarket (`bid_price > 0.0 && ask_price > 0.0 && bid_price >= ask_price`) bị bỏ qua hoàn toàn!

#### Preconditions
Bất kỳ bản tin Quote nào từ sàn hoặc feed dữ liệu có `bid_price = 0.0` và `bid_vol > 0.0`.

#### Steps to Reproduce
1. Gọi `Quote::new("HPG".to_string(), 0.0, 10000.0, 28000.0, 500.0, ts)`.
2. Quan sát kết quả trả về.
3. Gọi `Quote::new("HPG".to_string(), 28000.0, 500.0, 0.0, 10000.0, ts)`.
4. Quan sát kết quả trả về.

#### Expected Result
Hàm `Quote::new` phải từ chối với lỗi `MarketDataError::InvalidPrice` hoặc `InvalidVolume` vì giá bằng 0 thì khối lượng phải bằng 0.

#### Actual Result
Cả hai trường hợp đều trả về `Ok(Quote)` hợp lệ:
```text
Zero bid price with positive vol result: Ok(Quote { symbol: "HPG", bid_price: 0.0, bid_vol: 10000.0, ask_price: 28000.0, ask_vol: 500.0, ... })
Zero ask price with positive vol result: Ok(Quote { symbol: "HPG", bid_price: 28000.0, bid_vol: 500.0, ask_price: 0.0, ask_vol: 10000.0, ... })
```

#### Evidence
Tại [crates/domain/src/market.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/market.rs#L95-L106):
```rust
if bid_price == 0.0 && ask_price == 0.0 {
    return Err(MarketDataError::InvalidPrice(
        "Cả bid_price và ask_price không thể đồng thời bằng 0".to_string(),
    ));
}
if bid_price > 0.0 && ask_price > 0.0 && bid_price >= ask_price {
    return Err(MarketDataError::CrossedMarket { ... });
}
```

#### Impact
Bóp méo tính toán độ sâu sổ lệnh (order book depth), tính sai giá bình quân gia quyền (VWAP), tính sai mid-price (ví dụ `(0 + 28000) / 2 = 14000` làm giá giảm 50%), kích hoạt tín hiệu bán khẩn cấp sai lệch từ Signal Engine.

#### Root Cause
Thiếu invariant ràng buộc: `(price == 0.0 && vol == 0.0) || (price > 0.0 && vol > 0.0)`.

#### Suggested Fix
Thêm kiểm tra vào `Quote::new`:
```rust
if (bid_price == 0.0 && bid_vol > 0.0) || (bid_price > 0.0 && bid_vol == 0.0) {
    return Err(MarketDataError::InvalidPrice("bid_price và bid_vol phải cùng bằng 0 hoặc cùng lớn hơn 0".to_string()));
}
if (ask_price == 0.0 && ask_vol > 0.0) || (ask_price > 0.0 && ask_vol == 0.0) {
    return Err(MarketDataError::InvalidPrice("ask_price và ask_vol phải cùng bằng 0 hoặc cùng lớn hơn 0".to_string()));
}
```

#### Regression Risk
Cần cập nhật các test case mô phỏng kịch trần/kịch sàn để đảm bảo giá và volume cùng bằng 0.

---

### BUG-005 — `RiskLevelConfig::validate` bỏ qua kiểm tra `beta_min` khi `beta_max` là None, chấp nhận NaN/âm

**Severity:** HIGH

**Reason:**
Cấu hình rủi ro (`RiskProfilesConfig`) là phòng tuyến tối quan trọng bảo vệ vốn nhà đầu tư. Profile `risky` trong cấu hình mặc định có `beta_min = 1.25` và `beta_max = None`. Do lỗi logic trong `validate()`, bất kỳ giá trị âm nào (ví dụ `-999.0`) hoặc giá trị `NaN`/`Infinity` gán cho `beta_min` đều vượt qua bước thẩm định cấu hình lúc khởi động, gây lỗi tính toán rủi ro khi chạy.

**Status:** Open

**Module:** `crates/domain/src/config.rs`

**Detection Method:** Dynamic Testing (`crates/domain/tests/qa_domain_tests.rs::test_qa_risk_config_beta_min_unvalidated_when_beta_max_is_none`)

**Reproducibility:** Always

#### Description
Tại dòng 342 của `crates/domain/src/config.rs`:
```rust
match self.beta_min.is_some() && self.beta_max.is_some() {
    true => match self.beta_min.unwrap() <= self.beta_max.unwrap() {
        true => {}
        false => { return Err(ConfigError::ValidationError(...)); }
    },
    false => {} // BỎ QUA HOÀN TOÀN!
}
```
Nếu chỉ có `beta_min` (hoặc chỉ có `beta_max`), biểu thức logic đánh giá là `false` và không có bất kỳ bước kiểm tra nào được thực hiện trên giá trị của trường đó!

#### Preconditions
Khởi tạo hoặc tải file cấu hình chứa `beta_min = Some(-999.0)` hoặc `Some(f64::NAN)` với `beta_max = None`.

#### Steps to Reproduce
1. Tạo struct `RiskLevelConfig` với `beta_min = Some(-999.0)` và `beta_max = None`.
2. Gọi `config.validate()`.
3. Quan sát: Hàm trả về `Ok(())`.

#### Expected Result
Hàm `validate()` phải từ chối các giá trị beta không hợp lệ (NaN, Infinity, hoặc giá trị âm nếu yêu cầu hệ số beta dương).

#### Actual Result
Hàm thẩm định chấp nhận thành công:
```text
RiskLevelConfig with beta_min = -999.0 and beta_max = None: Ok(())
RiskLevelConfig with beta_min = NaN and beta_max = None: Ok(())
```

#### Evidence
Tại [crates/domain/src/config.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/config.rs#L341-L353).

#### Impact
Khi chạy Risk Engine, các cổ phiếu có beta âm hoặc dữ liệu NaN sẽ được phân loại sai vào nhóm `risky`, dẫn đến việc tự động mở vị thế mua sai quy tắc quản trị rủi ro.

#### Root Cause
Sử dụng toán tử `&&` thay vì kiểm tra tính hợp lệ độc lập của từng biến `beta_min`, `beta_max` trước khi kiểm tra mối quan hệ thứ tự giữa chúng.

#### Suggested Fix
```rust
if let Some(b_min) = self.beta_min {
    if !b_min.is_finite() || b_min < 0.0 {
        return Err(ConfigError::ValidationError("beta_min phải là số thực hữu hạn và >= 0".to_string()));
    }
}
if let Some(b_max) = self.beta_max {
    if !b_max.is_finite() || b_max < 0.0 {
        return Err(ConfigError::ValidationError("beta_max phải là số thực hữu hạn và >= 0".to_string()));
    }
}
if let (Some(b_min), Some(b_max)) = (self.beta_min, self.beta_max) {
    if b_min > b_max {
        return Err(ConfigError::ValidationError("beta_min không thể lớn hơn beta_max".to_string()));
    }
}
```

#### Regression Risk
Không có nguy cơ hồi quy (chỉ thắt chặt thẩm định cấu hình).

---

### BUG-006 — Hàm parse `StockSymbol::new` bất đối xứng và cho phép chuỗi rác phía sau (`HPG:GARBAGE`)

**Severity:** MEDIUM

**Reason:**
Gây lỗi lọt dữ liệu rác (data leakage) và hành vi bất nhất. Tùy thuộc vào vị trí của chuỗi rác (đứng trước hay đứng sau), hàm parse sẽ chấp nhận hoặc từ chối cùng một loại dữ liệu không hợp lệ.

**Status:** Open

**Module:** `crates/domain/src/symbol.rs`

**Detection Method:** Dynamic Testing (`crates/domain/tests/qa_domain_tests.rs::test_qa_stock_symbol_trailing_garbage_asymmetry`)

**Reproducibility:** Always

#### Description
Trong `StockSymbol::new`:
```rust
for part in symbol.split(|c| c == ':' || c == '.' || c == '/') {
    let trimmed = part.trim().to_uppercase();
    if EXCHANGES.contains(&trimmed.as_str()) {
        continue;
    }
    if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(...);
    }
    return Ok(Self { symbol: trimmed }); // RETURN NGAY LẬP TỨC!
}
```
Vòng lặp `for` trả về `Ok` ngay khi gặp phần tử hợp lệ đầu tiên mà không kiểm tra các phần tử còn lại của chuỗi. Do đó:
- `"HPG:GARBAGE"` -> Token 1 là `"HPG"` (hợp lệ) -> Return `Ok(StockSymbol { symbol: "HPG" })`.
- `"GARBAGE:HPG"` -> Token 1 là `"GARBAGE"` (không nằm trong `EXCHANGES`, độ dài != 3) -> Return `Err`.

#### Preconditions
Nhận chuỗi symbol có chứa chuỗi phụ phía sau dấu phân cách.

#### Steps to Reproduce
1. Gọi `StockSymbol::new("HPG:GARBAGE")`.
2. Quan sát kết quả: Trả về `Ok`.
3. Gọi `StockSymbol::new("GARBAGE:HPG")`.
4. Quan sát kết quả: Trả về `Err`.

#### Expected Result
Hàm phải thẩm định toàn bộ chuỗi: một phần là sàn giao dịch (`EXCHANGES`), phần còn lại là mã chứng khoán 3 ký tự. Nếu có phần thứ ba hoặc phần không xác định, phải từ chối.

#### Actual Result
```text
StockSymbol('HPG:GARBAGE') = Ok(StockSymbol { symbol: "HPG" })
StockSymbol('GARBAGE:HPG') = Err(InvalidData { symbol: "GARBAGE:HPG", reason: "Stock symbol must be 3 alphabetic characters" })
```

#### Evidence
Tại [crates/domain/src/symbol.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/symbol.rs#L35-L50).

#### Impact
Bản tin lỗi từ broker hoặc chuỗi định dạng sai như `HPG:ORDER123` bị nhận nhầm thành mã cổ phiếu sạch `HPG`.

#### Root Cause
Dùng vòng lặp `for part in split` và trả về sớm (`early return`) mà không kiểm tra độ dài mảng token sau khi split.

#### Suggested Fix
Tách chuỗi thành vector tokens, xác minh số lượng tokens tối đa là 2 (1 exchange + 1 ticker).

#### Regression Risk
Thấp. Cần kiểm tra lại các unit test của `StockSymbol`.

---

### BUG-007 — `Instrument::parse_canonical` thất bại đối với mã Phái sinh và Chỉ số có tiền tố sàn (`HOSE:VN30`)

**Severity:** MEDIUM

**Reason:**
Bất nhất giữa các loại công cụ tài chính (`Stock`, `Index`, `IndexFuture`). Trong khi cổ phiếu được hỗ trợ tiền tố sàn (`HOSE:HPG`), thì chỉ số và hợp đồng tương lai có tiền tố sàn (`HOSE:VN30`, `HNX:VN30F2409`) lại bị từ chối và báo lỗi.

**Status:** Open

**Module:** `crates/domain/src/symbol.rs`

**Detection Method:** Dynamic Testing (`crates/domain/tests/qa_domain_tests.rs::test_qa_instrument_parse_canonical_with_exchange_prefix`)

**Reproducibility:** Always

#### Description
Hàm `Instrument::parse_canonical` thực hiện so sánh chuỗi thô:
```rust
let s = s.trim().to_uppercase();
if s == "VN30" || s == "VNINDEX" {
    return Ok(Self::Index(IndexSymbol::new(&s)?));
} else if s.starts_with("VN30F") {
    return Ok(Self::IndexFuture(FutureContract::new(&s)?));
} else {
    return Ok(Self::Stock(StockSymbol::new(&s)?));
}
```
Nếu truyền vào `"HOSE:VN30"`, nó không khớp với `"VN30"` và cũng không bắt đầu bằng `"VN30F"`. Do đó, nó rơi vào nhánh `else` gọi `StockSymbol::new("HOSE:VN30")`. `StockSymbol` bóc tách được `"VN30"`, nhưng vì `"VN30"` có 4 ký tự và chứa số, `StockSymbol` từ chối với lỗi: `Stock symbol must be 3 alphabetic characters`.

#### Preconditions
Chuỗi dữ liệu đầu vào chứa tiền tố sàn trước mã chỉ số hoặc phái sinh.

#### Steps to Reproduce
1. Gọi `Instrument::parse_canonical("HOSE:VN30")`.
2. Gọi `Instrument::parse_canonical("HNX:VN30F2409")`.
3. Quan sát kết quả.

#### Expected Result
Cả hai trường hợp đều phải parse thành công thành `Instrument::Index` và `Instrument::IndexFuture`.

#### Actual Result
```text
parse_canonical('HOSE:VN30') = Err(InvalidData { symbol: "HOSE:VN30", reason: "Stock symbol must be 3 alphabetic characters" })
parse_canonical('HNX:VN30F2409') = Err(InvalidData { symbol: "HNX:VN30F2409", reason: "Stock symbol must be 3 alphabetic characters" })
```

#### Evidence
Tại [crates/domain/src/symbol.rs](file:///d:/rust/phân tích cổ phiếu/crates/domain/src/symbol.rs#L117-L126).

#### Impact
Hệ thống không thể tiếp nhận dữ liệu thị trường từ các broker có gửi kèm tiền tố sàn cho chỉ số VN30 hoặc phái sinh VN30F (ví dụ feed từ HNX/HOSE).

#### Root Cause
Xử lý bóc tách tiền tố sàn (`HOSE:`, `HNX:`) được cài đặt cục bộ bên trong `StockSymbol::new` thay vì xử lý chung ở tầng `Instrument::parse_canonical`.

#### Suggested Fix
Thực hiện strip tiền tố/hậu tố sàn trước tại hàm `Instrument::parse_canonical`, sau đó mới phân loại công cụ tài chính.

#### Regression Risk
Cần đảm bảo hàm `SymbolMapper` không bị ảnh hưởng.

---

### BUG-008 — Lỗi nghiệp vụ sàn (`ExchangeError`) bị map sai thành `ParseError` trong `try_into_market_event`

**Severity:** MEDIUM

**Reason:**
Gây nhầm lẫn phân loại lỗi trong tầng Observability và cơ chế tự phục hồi (Self-Healing). Lỗi do sàn trả về (ví dụ: chạm rate limit, token hết hạn, subscription không hợp lệ) bị coi là lỗi cú pháp JSON (`ParseError`).

**Status:** Open

**Module:** `crates/market-data/src/parser.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_exchange_error_misclassified_as_parse_error`)

**Reproducibility:** Always

#### Description
Tại dòng 96 của `crates/market-data/src/parser.rs`:
```rust
MarketMessage::ExchangeError(err) => {
    Err(MarketDataError::ParseError(err.message.clone()))
}
```
Bản tin đã được parse JSON hoàn toàn hợp lệ thành biến thể `MarketMessage::ExchangeError`. Tuy nhiên khi chuyển đổi sang sự kiện thị trường, nó lại bị ném thành `MarketDataError::ParseError`.

#### Preconditions
Nhận frame lỗi từ server WebSocket (ví dụ: `{"type": "error", "code": "RATE_LIMIT", "message": "Too many requests"}`).

#### Steps to Reproduce
1. Parse JSON trên qua `MarketDataParser::parse_json`.
2. Gọi `try_into_market_event()` trên message thu được.
3. Quan sát lỗi trả về.

#### Expected Result
Hệ thống phải trả về lỗi chuyên biệt phản ánh lỗi từ sàn, ví dụ `MarketDataError::ConnectionError` hoặc biến thể mới `MarketDataError::ExchangeError { code, message }`.

#### Actual Result
Trả về `MarketDataError::ParseError("Too many requests to exchange")`.

#### Evidence
Tại [crates/market-data/src/parser.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/parser.rs#L96-L98).

#### Impact
Hệ thống không thể phân biệt giữa lỗi dữ liệu hỏng (corrupted payload) cần drop bỏ và lỗi sàn bị nghẽn (rate limited) cần kích hoạt backoff retry.

#### Root Cause
Thiếu biến thể lỗi tương ứng trong `MarketDataError`.

#### Suggested Fix
Thêm `ExchangeError { code: String, message: String }` vào enum `MarketDataError` trong `crates/domain/src/errors.rs`.

#### Regression Risk
Cần cập nhật các `match` pattern trên `MarketDataError`.

---

### BUG-009 — Cấu hình `dedup_trades` bị bỏ qua hoàn toàn trong `EventDeduplicator::is_duplicate`

**Severity:** MEDIUM

**Reason:**
Dead configuration và vi phạm nguyên tắc thiết kế API. Cung cấp tham số `dedup_trades: bool` trong constructor và hàm getter `dropped_trades()`, nhưng logic nghiệp vụ bên trong hardcode bỏ qua không bao giờ thực hiện.

**Status:** Open

**Module:** `crates/market-data/src/dedup.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_dedup_trades_flag_completely_ignored`)

**Reproducibility:** Always

#### Description
Hàm khởi tạo `EventDeduplicator::new(capacity: usize, dedup_trades: bool)` nhận tham số `dedup_trades`. Tuy nhiên trong hàm `is_duplicate`:
```rust
match event {
    MarketEvent::Quote(quote) => { ... }
    MarketEvent::Trade(_) => {
        return false;
    }
}
```
Nhánh `MarketEvent::Trade(_)` lập tức trả về `false` mà không hề kiểm tra `self.dedup_trades`. Trường `self.dropped_trades` không bao giờ được cập nhật.

#### Preconditions
Khởi tạo `EventDeduplicator::new(100, true)`.

#### Steps to Reproduce
1. Gửi 2 sự kiện `MarketEvent::Trade` giống hệt nhau liên tiếp vào `is_duplicate`.
2. Quan sát kết quả lần thứ hai.

#### Expected Result
Nếu `dedup_trades == true`, phải thực hiện kiểm tra dedup cho trade (dựa trên sequence number hoặc fingerprint); nếu `dedup_trades == false`, trả về `false`. Nếu dự án quyết định KHÔNG BAO GIỜ dedup trade, phải loại bỏ tham số này khỏi API.

#### Actual Result
Trade giống hệt nhau lần 2 vẫn trả về `false` và `dropped_trades()` bằng 0.

#### Evidence
Tại [crates/market-data/src/dedup.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/dedup.rs#L67-L70).

#### Impact
Gây hiểu lầm cho lập trình viên và người vận hành khi cấu hình `dedup_trades = true` nhưng hệ thống không có bất kỳ hành động nào.

#### Root Cause
Quyết định kiến trúc ngày 2026-09-17 chỉ dedup Quote nhưng không refactor lại API struct `EventDeduplicator`.

#### Suggested Fix
Lựa chọn 1 trong 2:
- Phương án A (Khuyên nghị): Xóa tham số `dedup_trades` và trường `dropped_trades` khỏi `EventDeduplicator` để API phản ánh đúng thiết kế "chỉ dedup Quote".
- Phương án B: Hoàn thiện logic dedup Trade khi cờ `dedup_trades == true` (sử dụng Trade ID/Sequence Number thay vì price/vol).

#### Regression Risk
Ảnh hưởng đến các unit test gọi `EventDeduplicator::new`.

---

### BUG-010 — Số thực âm không (`-0.0`) tạo fingerprint khác `+0.0`, làm lọt Quote trùng lặp qua bộ dedup

**Severity:** MEDIUM

**Reason:**
Sai lệch trong thuật toán băm (hashing). Trong chuẩn IEEE 754, `+0.0` và `-0.0` có cùng giá trị số học (`0.0 == -0.0`), nhưng bit biểu diễn khác nhau hoàn toàn (`0x0` so với `0x8000000000000000`). Vì hàm `Quote::new` chấp nhận `-0.0` (do `-0.0 < 0.0` là `false`), hai bản tin quote giống hệt nhau về mặt giá trị số học sẽ sinh ra 2 mã băm khác nhau, khiến bộ lọc dedup không nhận diện được trùng lặp.

**Status:** Open

**Module:** `crates/market-data/src/dedup.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_dedup_negative_zero_hash_discrepancy`)

**Reproducibility:** Always

#### Description
Trong `EventDeduplicator::fingerprint`:
```rust
quote.bid_price.to_bits().hash(&mut hasher);
quote.bid_vol.to_bits().hash(&mut hasher);
quote.ask_price.to_bits().hash(&mut hasher);
quote.ask_vol.to_bits().hash(&mut hasher);
```
Khi `ask_price = 0.0`, `to_bits()` cho giá trị `0`. Khi `ask_price = -0.0`, `to_bits()` cho giá trị `9223372036854775808` (`0x8000000000000000`). Hai mã băm sinh ra là:
`q1 (0.0): 3d3873becff42c01`
`q2 (-0.0): 8516879f4aea9d9a`
Dẫn đến quote thứ hai không bị loại bỏ.

#### Preconditions
Nhận Quote có trường giá hoặc khối lượng mang giá trị `-0.0`.

#### Steps to Reproduce
1. Tạo `q1` với `ask_price = 0.0`.
2. Tạo `q2` với `ask_price = -0.0`.
3. Đẩy `q1` rồi `q2` vào `EventDeduplicator`.
4. Quan sát: `is_duplicate(&q2)` trả về `false`.

#### Expected Result
Bộ dedup phải nhận diện `q2` trùng lặp với `q1`.

#### Actual Result
`is_duplicate` trả về `false`.

#### Evidence
Tại [crates/market-data/src/dedup.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/dedup.rs#L34-L43).

#### Impact
Các quote trùng lặp lọt qua bộ lọc, gây lãng phí tài nguyên CPU tính toán downstream.

#### Root Cause
Không chuẩn hóa số thực (`if val == 0.0 { 0.0f64 } else { val }`) trước khi gọi `to_bits()`.

#### Suggested Fix
Trong hàm `fingerprint`:
```rust
fn canonical_bits(v: f64) -> u64 {
    if v == 0.0 { 0.0f64.to_bits() } else { v.to_bits() }
}
```

#### Regression Risk
Không có nguy cơ hồi quy.

---

### BUG-011 — `register_alias` không kiểm tra tính hợp lệ của mã đích (canonical) lúc đăng ký

**Severity:** LOW

**Reason:**
Thiếu nguyên tắc Fail-Fast khi nạp cấu hình alias. Lỗi chỉ phát lộ vào giờ giao dịch thực tế khi nhận được tick thị trường.

**Status:** Open

**Module:** `crates/market-data/src/symbol_mapper.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_symbol_mapper_allows_invalid_canonical_alias`)

**Reproducibility:** Always

#### Description
Hàm `SymbolMapper::register_alias(&mut self, alias: &str, canonical: &str)` nhận trực tiếp chuỗi `canonical` và lưu vào `HashMap` mà không gọi `Instrument::parse_canonical` để kiểm tra. Nếu lập trình viên cấu hình nhầm một mã không hợp lệ, hệ thống khởi động bình thường nhưng sẽ ném lỗi runtime khi nhận được message đầu tiên liên quan đến alias đó.

#### Preconditions
Gọi `register_alias("VN30F1M", "NOT_A_VALID_TICKER_123")`.

#### Steps to Reproduce
1. Đăng ký alias với mã đích sai.
2. Gọi `map("VN30F1M")`.
3. Quan sát kết quả: Runtime error.

#### Expected Result
`register_alias` phải trả về `Result<(), MarketDataError>` và từ chối ngay tại thời điểm đăng ký.

#### Actual Result
Đăng ký thành công âm thầm, runtime lỗi khi map tick.

#### Evidence
Tại [crates/market-data/src/symbol_mapper.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/symbol_mapper.rs#L18-L22).

#### Impact
Bản tin thị trường bị drop hoặc gây panic trong giờ giao dịch nếu file cấu hình alias bị gõ sai chính tả.

#### Root Cause
Thiếu validation tại biên nạp cấu hình.

#### Suggested Fix
```rust
pub fn register_alias(&mut self, alias: &str, canonical: &str) -> Result<(), MarketDataError> {
    let canonical_inst = Instrument::parse_canonical(canonical)?;
    self.aliases.insert(alias.trim().to_uppercase(), canonical_inst.as_str().to_string());
    Ok(())
}
```

#### Regression Risk
Cần cập nhật các vị trí gọi `register_alias` để xử lý `Result`.

---

### BUG-012 — Hiện tượng lệch đồng hồ hệ thống (NTP skew) gây báo cáo sai trạng thái `Healthy`

**Severity:** LOW

**Reason:**
Sử dụng phép trừ bão hòa `current_time_ms.saturating_sub(message_ts)` khiến cho khi đồng hồ hệ thống bị điều chỉnh lùi (do đồng bộ NTP) hoặc timestamp nhận được bị sai lệch, `message_age` luôn bằng 0 và báo cáo trạng thái `Healthy` ngay cả khi kết nối đã chết. Đồng thời, trường `last_heartbeat_ts` trong `HealthMonitor` không bao giờ được sử dụng.

**Status:** Open

**Module:** `crates/market-data/src/health.rs`

**Detection Method:** Dynamic Testing (`crates/market-data/tests/qa_market_data_tests.rs::test_qa_health_monitor_clock_skew_or_out_of_order`) + Static Analysis

**Reproducibility:** Always

#### Description
Trong `HealthMonitor::check_health`:
```rust
let message_age = current_time_ms.saturating_sub(message_ts);
```
Nếu `current_time_ms < message_ts` (ví dụ server nhận bản tin có timestamp đi trước do lệch giờ, hoặc đồng hồ máy tính vừa bị NTP lùi lại 5 giây), `message_age` cho kết quả `0`. Hệ thống đánh giá là `Healthy` dù thực tế kết nối có thể đã ngưng trệ. Đồng thời, struct lưu `last_heartbeat_ts` nhưng hàm kiểm tra hoàn toàn không đọc đến trường này.

#### Preconditions
Đồng hồ hệ thống chạy lùi hoặc `current_time_ms < last_message_ts`.

#### Steps to Reproduce
1. Ghi nhận `record_message(20_000)`.
2. Kiểm tra sức khỏe tại thời điểm `15_000`.
3. Quan sát: Trả về `Healthy`.

#### Expected Result
Cần phát hiện bất thường về thời gian hoặc sử dụng đồng hồ đơn điệu (`Instant`) thay vì wall-clock timestamp để tính tuổi thọ kết nối.

#### Actual Result
Báo cáo `HealthStatus::Healthy`.

#### Evidence
Tại [crates/market-data/src/health.rs](file:///d:/rust/phân tích cổ phiếu/crates/market-data/src/health.rs#L43-L65).

#### Impact
Chậm trễ trong việc phát hiện đứt kết nối ngầm khi xảy ra hiện tượng NTP sync.

#### Root Cause
Sử dụng epoch milliseconds dạng số nguyên thay vì `std::time::Instant` cho các phép đo khoảng thời gian (duration/timeout).

#### Suggested Fix
Chuyển sang sử dụng `std::time::Instant` để đo timeout thay vì Unix epoch timestamp.

#### Regression Risk
Thấp.

---

## 6. Potential Issues

| ID | Module | Issue | Risk | Reason |
| :--- | :--- | :--- | :--- | :--- |
| **POT-001** | `market-data::dedup` | Nguy cơ xung đột mã băm 64-bit (Hash Collision) làm mất Quote hợp lệ | MEDIUM | `EventDeduplicator` lưu trữ mã băm `u64` trong `HashSet<u64>` thay vì lưu struct Quote hoặc composite key; nếu 2 Quote khác nhau vô tình có cùng mã băm 64-bit, Quote thứ hai sẽ bị drop vĩnh viễn. |
| **POT-002** | `market-data::websocket` | Thiếu backpressure handling hoặc giới hạn kích thước message thô | MEDIUM | `RawMarketMessage` không giới hạn kích thước payload; nếu server gửi frame dung lượng lớn bất thường hoặc tấn công DoS, có thể gây tràn RAM bộ đệm channel. |
| **POT-003** | `market-data::reconnect` | Cờ `biased;` trong `tokio::select!` có thể gây đói tác vụ (starvation) kiểm tra sức khỏe | LOW | Khi lượng bản tin thị trường cực lớn liên tục sẵn sàng trên `ws_read.next()`, nhánh `health_interval.tick()` có thể bị trì hoãn kiểm tra trong thời gian dài. |

---

## 7. Code Quality / Improvement Suggestions

1. **Strict Case Sensitivity trong `ServerConfig`:**
   - Trong `ServerConfig::validate()`, chỉ chấp nhận chính xác `"development"`, `"info"` chữ thường. Nên áp dụng `.trim().to_lowercase()` để hỗ trợ các biến môi trường phổ biến như `"PRODUCTION"`, `"INFO"`, `"DEBUG"`.
2. **Xử lý mã lỗi dạng số trong `ExchangeErrorEvent`:**
   - Trường `code: String` trong `ExchangeErrorEvent` sẽ khiến Serde báo lỗi cú pháp nếu sàn chứng khoán trả về mã lỗi dạng số nguyên (ví dụ: `{"code": 401, "message": "Unauthorized"}`). Nên cấu hình custom deserializer để chấp nhận cả chuỗi lẫn số nguyên.
3. **Kiến trúc `WebSocketClient` và `MarketConnectionManager`:**
   - `WebSocketClient::connect` mở socket và trả về `(Receiver, JoinHandle)` nhưng không trả về sink gửi tin và không được dùng trong `MarketConnectionManager` (bị lặp code gọi `connect_async`). Nên hợp nhất hoặc làm rõ trách nhiệm giữa tầng transport và tầng session manager.
4. **Loại bỏ Dead Fields:**
   - `HealthMonitor.last_heartbeat_ts` được ghi nhận nhưng không bao giờ được đọc. Nên tận dụng để kiểm tra độc lập giữa heartbeat timeout và tick staleness timeout.

---

## 8. Security Findings

| ID | Vulnerability | Severity | Affected Component | Evidence |
| :--- | :--- | :--- | :--- | :--- |
| **SEC-001** | Serde Invariant Bypass / Data Integrity Violation | **MEDIUM** | `domain::timestamp`, `domain::market` | JSON deserialization trực tiếp nạp ngày tháng năm 1970/3000, giá âm, khối lượng âm và crossed markets vào bộ nhớ. |
| **SEC-002** | Thiếu kích thước tối đa cho WebSocket incoming frames | **LOW** | `market-data::websocket` | Không giới hạn `max_message_size` trên tungstenite connection, có nguy cơ cạn kiệt bộ nhớ nếu nhận frame độc hại. |
| **SEC-003** | Nguy cơ lộ Bot Token trong log nếu debug struct | **LOW** | `domain::config` | `TelegramConfig` derive `Debug` có thể in biến môi trường token ra log; cần đảm bảo token luôn được redact khi ghi log. |

---

## 9. Test Cases

| ID | Module | Test Case | Expected | Actual | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **TC-001** | `domain::timestamp` | Epoch millis hợp lệ năm 2024 | Tạo timestamp thành công | Khớp ngày giờ | **PASS** |
| **TC-002** | `domain::timestamp` | Epoch secs hợp lệ | Tạo timestamp thành công | Khớp giây | **PASS** |
| **TC-003** | `domain::timestamp` | Từ chối timestamp <= 0 | Báo lỗi `InvalidTimestamp` | Trả về `Err` | **PASS** |
| **TC-004** | `domain::timestamp` | Từ chối năm < 2000 và > 2100 | Báo lỗi `InvalidTimestamp` | Trả về `Err` | **PASS** |
| **TC-005** | `domain::timestamp` | Chuyển đổi múi giờ Việt Nam (UTC+7) | Giờ tăng 7 tiếng | Khớp giờ VN | **PASS** |
| **TC-006** | `domain::timestamp` | Serde deserialize timestamp năm 1970 | Từ chối do vi phạm invariant | Bị bypass (năm 1970) | **BUG-003** |
| **TC-007** | `domain::market` | Tạo Trade hợp lệ chuẩn hóa hoa | Trả về Trade sạch | Khớp symbol & price | **PASS** |
| **TC-008** | `domain::market` | Trade với giá âm, zero, NaN, Inf | Báo lỗi `InvalidPrice` | Trả về `Err` | **PASS** |
| **TC-009** | `domain::market` | Trade với volume âm, zero, NaN | Báo lỗi `InvalidVolume` | Trả về `Err` | **PASS** |
| **TC-010** | `domain::market` | Serde deserialize Trade giá âm | Từ chối do vi phạm invariant | Bị bypass (giá -9999) | **BUG-003** |
| **TC-011** | `domain::market` | Quote kịch trần (ask price = 0, ask vol = 0) | Chấp nhận hợp lệ | Trả về `Ok` | **PASS** |
| **TC-012** | `domain::market` | Quote kịch sàn (bid price = 0, bid vol = 0) | Chấp nhận hợp lệ | Trả về `Ok` | **PASS** |
| **TC-013** | `domain::market` | Quote crossed market (`bid >= ask`) | Báo lỗi `CrossedMarket` | Trả về `Err` | **PASS** |
| **TC-014** | `domain::market` | Quote có bid = 0 nhưng bid vol = 10,000 | Phải từ chối vì giá 0 vol > 0 | Bị chấp nhận (Ok) | **BUG-004** |
| **TC-015** | `domain::market` | Quote có ask = 0 nhưng ask vol = 10,000 | Phải từ chối vì giá 0 vol > 0 | Bị chấp nhận (Ok) | **BUG-004** |
| **TC-016** | `domain::symbol` | Parse stock ticker hợp lệ `HPG` | Tạo `StockSymbol` sạch | Khớp "HPG" | **PASS** |
| **TC-017** | `domain::symbol` | Parse stock có tiền tố/hậu tố sàn (`HOSE:HPG`) | Bóc tách sàn thành công | Khớp "HPG" | **PASS** |
| **TC-018** | `domain::symbol` | Parse stock có chuỗi rác sau (`HPG:GARBAGE`) | Phải từ chối toàn bộ chuỗi | Bị chấp nhận ("HPG") | **BUG-006** |
| **TC-019** | `domain::symbol` | Parse future canonical `VN30F2409` | Phân tách năm 24 tháng 09 | Khớp canonical | **PASS** |
| **TC-020** | `domain::symbol` | Parse future tháng không hợp lệ (tháng 0, 13) | Báo lỗi `InvalidData` | Trả về `Err` | **PASS** |
| **TC-021** | `domain::symbol` | Parse Instrument `HOSE:VN30` | Phải bóc tách sàn ra `VN30` | Báo lỗi (Stock 3 char) | **BUG-007** |
| **TC-022** | `domain::symbol` | Parse Instrument `HNX:VN30F2409` | Phải bóc tách sàn ra Future | Báo lỗi (Stock 3 char) | **BUG-007** |
| **TC-023** | `domain::config` | Thẩm định file `config.example.toml` | Thẩm định thành công | Trả về `Ok` | **PASS** |
| **TC-024** | `domain::config` | RiskLevelConfig với `beta_min = -999.0`, `beta_max = None` | Phải từ chối vì beta âm | Bị chấp nhận (Ok) | **BUG-005** |
| **TC-025** | `domain::config` | RiskLevelConfig với `beta_min = NaN`, `beta_max = None` | Phải từ chối vì NaN | Bị chấp nhận (Ok) | **BUG-005** |
| **TC-026** | `market-data::auth` | Auth method None | Trả về `None` payload | Khớp None | **PASS** |
| **TC-027** | `market-data::auth` | ApiKey rỗng hoặc khoảng trắng | Báo lỗi `AuthenticationError` | Trả về `Err` | **PASS** |
| **TC-028** | `market-data::auth` | Bearer token hợp lệ tạo JSON | Tạo đúng JSON schema | Khớp payload | **PASS** |
| **TC-029** | `market-data::auth` | Verify phản hồi status ok | Trả về `Ok(true)` | Trả về true | **PASS** |
| **TC-030** | `market-data::subscription` | Chuẩn hóa ticker chữ hoa, trim | Chuẩn hóa sạch | Khớp ticker | **PASS** |
| **TC-031** | `market-data::subscription` | Đăng ký trùng lặp không sinh frame thừa | Trả về `None` | Trả về `None` | **PASS** |
| **TC-032** | `market-data::subscription` | Đăng ký chuỗi ký tự rác `!@#$%^&*` | Phải từ chối mã không hợp lệ | Bị chấp nhận (Ok) | **BUG-011** |
| **TC-033** | `market-data::parser` | Parse JSON trade, quote, heartbeat | Phân loại đúng enum variant | Khớp variant | **PASS** |
| **TC-034** | `market-data::parser` | Chuyển đổi `ExchangeError` sang event | Trả về lỗi chuyên biệt sàn | Trả về `ParseError` | **BUG-008** |
| **TC-035** | `market-data::health` | Khởi tạo trạng thái ban đầu | Trả về `Dead` | Trả về `Dead` | **PASS** |
| **TC-036** | `market-data::health` | Cập nhật heartbeat định kỳ | Trả về `Healthy` | Trả về `Healthy` | **PASS** |
| **TC-037** | `market-data::health` | Quá hạn heartbeat nhưng chưa staleness | Trả về `HeartbeatMissed` | Trả về `HeartbeatMissed`| **PASS** |
| **TC-038** | `market-data::health` | Lệch đồng hồ (clock skew ngược thời gian) | Cảnh báo hoặc đo đơn điệu | Báo `Healthy` ảo | **BUG-012** |
| **TC-039** | `market-data::reconnect` | Tính toán exponential backoff và max cap | Tăng theo lũy thừa, kịch trần | Khớp số ms | **PASS** |
| **TC-040** | `market-data::reconnect` | Tự ngắt khi đạt `max_retries = Some(1)` sau 1 lần đứt kết nối | Phải thử kết nối lại | Bị terminate vĩnh viễn | **BUG-002** |
| **TC-041** | `market-data::reconnect` | Phản hồi Ping từ WebSocket Server | Gửi lại Pong frame | Không gửi Pong, timeout | **BUG-001** |
| **TC-042** | `market-data::symbol_mapper` | Map future qua alias `VN30F1M` -> `VN30F2409` | Trả về mã tương lai đích | Khớp canonical | **PASS** |
| **TC-043** | `market-data::symbol_mapper` | Đăng ký alias với mã canonical không hợp lệ | Phải báo lỗi ngay lúc đăng ký | Âm thầm nhận, lỗi sau | **BUG-011** |
| **TC-044** | `market-data::dedup` | Quote giống hệt nhau bị loại bỏ | Lần 1 `false`, lần 2 `true` | Khớp kết quả | **PASS** |
| **TC-045** | `market-data::dedup` | Quote khác giá, vol, hoặc timestamp | Không bị coi là duplicate | Đều trả về `false` | **PASS** |
| **TC-046** | `market-data::dedup` | Cấu hình `dedup_trades = true` | Phải dedup hoặc không hỗ trợ | Bị bỏ qua hoàn toàn | **BUG-009** |
| **TC-047** | `market-data::dedup` | Quote chứa `-0.0` và `+0.0` | Nhận diện là duplicate | Sinh hash khác nhau | **BUG-010** |

---

## 10. Final Assessment

1. **Về Test Coverage:** 
   Bộ test hiện tại đã bao phủ 100% các hàm nghiệp vụ chính từ M0 đến M3-T05 (147 test cases).
2. **Về Độ ổn định & Sẵn sàng (Production Readiness):** 
   **CHƯA ĐẠT YÊU CẦU ĐỂ TRIỂN KHAI HOẶC CHUYỂN TIẾP MILESTONE TIẾP THEO.**
   Dự án đang tồn tại **2 lỗi nghiêm trọng (CRITICAL)** và **3 lỗi mức cao (HIGH)**:
   - Hệ thống kết nối WebSocket (`market-data::reconnect`) sẽ bị sàn đóng kết nối cưỡng bức do không phản hồi WebSocket Ping và sẽ dừng chạy vĩnh viễn khi gặp lỗi mạng đầu tiên nếu cấu hình `max_retries`.
   - Tầng chuẩn hóa dữ liệu (`domain::market`) chấp nhận các Quote phi lý (bán cổ phiếu giá 0 đồng, mua giá 0 đồng) và cho phép deserialize dữ liệu rác phá vỡ toàn bộ Domain Invariants.
3. **Ưu tiên xử lý của Developer:**
   - **Ưu tiên 1 (P0 - Khẩn cấp):** Sửa **BUG-001** (giữ lại write sink để phản hồi Ping/Pong và hỗ trợ đăng ký động) và **BUG-002** (reset biến `attempt = 0` sau khi kết nối thành công để vòng lặp Reconnect hoạt động chính xác).
   - **Ưu tiên 2 (P1 - Nghiêm trọng):** Sửa **BUG-003** (bảo vệ Domain Invariant khi Serde deserialize) và **BUG-004** (ràng buộc `bid_price == 0.0 <=> bid_vol == 0.0` trong `Quote::new`).
   - **Ưu tiên 3 (P2 - Trung bình):** Sửa **BUG-005** (`beta_min` validation trong `RiskLevelConfig`), **BUG-007** (strip sàn cho Index và Future), và **BUG-009**/**BUG-010** (chuẩn hóa `-0.0` và làm sạch API `EventDeduplicator`).
