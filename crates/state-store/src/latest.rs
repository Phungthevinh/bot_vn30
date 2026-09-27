use dashmap::DashMap;
use tracing;

use vn30_domain::errors::PriceDataError;
use vn30_domain::market::{MarketEvent, Quote, Trade};
use vn30_domain::timestamp::MarketTimestamp;

/// Trạng thái thị trường thời gian thực của một mã chứng khoán đơn lẻ trong rổ VN30 hoặc Phái sinh.
///
/// Duy trì tính độc lập (self-contained) và phản ánh chính xác lát cắt hiện tại (snapshot):
/// - Lệnh khớp gần nhất (`last_trade`)
/// - Sổ lệnh giá mua/bán tốt nhất gần nhất (`last_quote`)
/// - Khối lượng giao dịch tích lũy từ đầu ngày (`total_volume`)
/// - Mốc thời gian của sự kiện mới nhất (`updated_at`)
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolState {
    /// Mã chứng khoán chuẩn hóa viết hoa (ví dụ: `"HPG"`, `"VN30F2409"`).
    pub symbol: String,

    /// Lệnh khớp gần nhất (`None` nếu mã chưa phát sinh giao dịch khớp lệnh nào trong ngày).
    pub last_trade: Option<Trade>,

    /// Mức giá chào mua / chào bán tốt nhất gần nhất (`None` nếu chưa nhận được sổ lệnh BBO).
    pub last_quote: Option<Quote>,

    /// Tổng khối lượng cổ phiếu/hợp đồng đã giao dịch tích lũy trong phiên (cộng dồn từ các lệnh Trade).
    pub total_volume: f64,

    /// Mốc thời gian của sự kiện cập nhật mới nhất (đảm bảo tính đơn điệu tăng dần).
    pub updated_at: MarketTimestamp,
}

impl SymbolState {
    /// Khởi tạo trạng thái ban đầu cho một mã chứng khoán với sổ lệnh và khớp lệnh rỗng.
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán chuẩn hóa.
    /// - `timestamp`: Mốc thời gian khởi tạo (thường là thời điểm nạp mã vào hệ thống).
    pub fn new(symbol: String, timestamp: MarketTimestamp) -> Self {
        Self {
            symbol,
            last_quote: None,
            last_trade: None,
            total_volume: 0.0,
            updated_at: timestamp,
        }
    }

    /// Cập nhật trạng thái khi nhận được một lệnh khớp mới (`Trade`).
    ///
    /// # Quy tắc nghiệp vụ & Bất biến (Invariants):
    /// - **Chặn sự kiện muộn (Stale Event)**: Nếu timestamp của lệnh khớp mới cũ hơn lệnh khớp hiện tại
    ///   (`trade.timestamp < last_trade.timestamp`), lệnh này sẽ bị từ chối với lỗi [`PriceDataError::StaleData`].
    /// - **Cộng dồn khối lượng**: Khối lượng thực tế `trade.volume` được cộng dồn vào `total_volume`.
    /// - **Đơn điệu thời gian**: `updated_at` được cập nhật tới mốc thời gian lớn hơn (`max`).
    ///
    /// # Lỗi trả về:
    /// - [`PriceDataError::StaleData`]: Nếu lệnh khớp mang mốc thời gian cũ hơn lệnh hiện có.
    pub fn update_trade(&mut self, trade: Trade) -> Result<(), PriceDataError> {
        // Kiểm tra xem lệnh khớp mới có bị trễ so với lệnh khớp trước đó hay không
        if let Some(ref last) = self.last_trade {
            if trade.timestamp < last.timestamp {
                return Err(PriceDataError::StaleData(
                    trade.timestamp.timestamp_millis(),
                ));
            }
        }

        // Cập nhật giá khớp, cộng dồn khối lượng thực tế và duy trì mốc thời gian mới nhất
        self.updated_at = std::cmp::max(self.updated_at, trade.timestamp);
        self.total_volume += trade.volume;
        self.last_trade = Some(trade);

        Ok(())
    }

    /// Cập nhật trạng thái khi nhận được bước giá chào mua/chào bán mới (`Quote`).
    ///
    /// # Quy tắc nghiệp vụ & Bất biến (Invariants):
    /// - **Chặn sự kiện muộn (Stale Event)**: Nếu timestamp của Quote mới cũ hơn Quote hiện tại
    ///   (`quote.timestamp < last_quote.timestamp`), sự kiện sẽ bị từ chối với lỗi [`PriceDataError::StaleData`].
    /// - **Đơn điệu thời gian**: `updated_at` được cập nhật tới mốc thời gian lớn hơn (`max`).
    ///
    /// # Lỗi trả về:
    /// - [`PriceDataError::StaleData`]: Nếu sổ lệnh mới mang mốc thời gian cũ hơn sổ lệnh hiện có.
    pub fn update_quote(&mut self, quote: Quote) -> Result<(), PriceDataError> {
        // Kiểm tra xem bước giá mới có bị trễ so với bước giá trước đó hay không
        if let Some(ref last) = self.last_quote {
            if quote.timestamp < last.timestamp {
                return Err(PriceDataError::StaleData(
                    quote.timestamp.timestamp_millis(),
                ));
            }
        }

        // Cập nhật sổ lệnh và duy trì mốc thời gian mới nhất
        self.updated_at = std::cmp::max(self.updated_at, quote.timestamp);
        self.last_quote = Some(quote);

        Ok(())
    }
}

/// Bộ lưu trữ trạng thái thị trường toàn cục cho toàn bộ rổ VN30 và Phái sinh trong RAM.
///
/// Sử dụng cấu trúc [`DashMap`] với cơ chế **Khóa phân mảnh (Sharded Locking)**:
/// - Cho phép hàng chục luồng đồng thời đọc/ghi các mã chứng khoán khác nhau mà không gây khóa nghẽn (Lock Contention).
/// - Đảm bảo tốc độ tra cứu $O(1)$ phục vụ cho Risk Engine, Signal Engine và Alert Bot.
#[derive(Debug, Default)]
pub struct MarketStateStore {
    /// Bảng băm đa luồng phân mảnh ánh xạ `symbol -> SymbolState`.
    states: DashMap<String, SymbolState>,
}

impl MarketStateStore {
    /// Khởi tạo một bộ nhớ trạng thái thị trường mới với bảng lưu trữ rỗng.
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }

    /// Cập nhật một sự kiện thị trường bất kỳ ([`MarketEvent`]) vào bộ nhớ RAM.
    ///
    /// Tự động định tuyến (route) sự kiện tới nhánh xử lý tương ứng (`Trade` hoặc `Quote`).
    /// Nếu mã chứng khoán chưa tồn tại trong bộ nhớ, nó sẽ được tự động khởi tạo mới.
    ///
    /// Nếu sự kiện bị trễ (`StaleData`), hàm sẽ ghi log cảnh báo [`tracing::warn!`] và bỏ qua an toàn.
    pub fn update_event(&self, event: &MarketEvent) {
        match event {
            MarketEvent::Trade(trade) => {
                let symbol_key = trade.symbol.trim().to_uppercase();
                let mut entry = self
                    .states
                    .entry(symbol_key)
                    .or_insert_with(|| SymbolState::new(trade.symbol.clone(), trade.timestamp));

                if let Err(err) = entry.update_trade(trade.clone()) {
                    tracing::warn!(
                        symbol = %trade.symbol,
                        error = %err,
                        "Bỏ qua lệnh khớp Trade đến muộn (stale) trong State Store"
                    );
                }
            }
            MarketEvent::Quote(quote) => {
                let symbol_key = quote.symbol.trim().to_uppercase();
                let mut entry = self
                    .states
                    .entry(symbol_key)
                    .or_insert_with(|| SymbolState::new(quote.symbol.clone(), quote.timestamp));

                if let Err(err) = entry.update_quote(quote.clone()) {
                    tracing::warn!(
                        symbol = %quote.symbol,
                        error = %err,
                        "Bỏ qua bước giá Quote đến muộn (stale) trong State Store"
                    );
                }
            }
        }
    }

    /// Tra cứu bản sao trạng thái thời gian thực của một mã chứng khoán.
    ///
    /// Tự động cắt tỉa khoảng trắng và chuẩn hóa ký tự viết hoa trước khi tra cứu.
    ///
    /// # Giá trị trả về:
    /// - `Some(SymbolState)`: Bản chụp trạng thái hiện tại của mã.
    /// - `None`: Nếu mã chứng khoán chưa từng phát sinh dữ liệu trong phiên.
    pub fn get_state(&self, symbol: &str) -> Option<SymbolState> {
        let symbol_normalized = symbol.trim().to_uppercase();
        self.states.get(&symbol_normalized).map(|entry| entry.value().clone())
    }

    /// Lấy nhanh mức giá khớp lệnh gần nhất của một mã chứng khoán.
    ///
    /// # Giá trị trả về:
    /// - `Some(f64)`: Mức giá khớp mới nhất.
    /// - `None`: Nếu chưa có dữ liệu của mã hoặc mã chưa phát sinh lệnh khớp nào.
    pub fn get_last_price(&self, symbol: &str) -> Option<f64> {
        self.get_state(symbol).and_then(|state| state.last_trade.map(|trade| trade.price))
    }

    /// Chụp ảnh (snapshot) toàn bộ trạng thái thị trường của tất cả các mã đang theo dõi.
    ///
    /// Thường được gọi bởi các tác vụ định kỳ (Periodic Scheduler), Bot Telegram hiển thị bảng giá,
    /// hoặc các thuật toán quét tín hiệu toàn bộ rổ VN30.
    pub fn snapshot_all(&self) -> Vec<SymbolState> {
        self.states.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Trả về tổng số mã chứng khoán đang được theo dõi và lưu trữ trong bộ nhớ.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Kiểm tra xem bộ nhớ trạng thái có đang rỗng hay không.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use vn30_domain::errors::PriceDataError;
    use vn30_domain::market::{MarketEvent, Quote, Trade};
    use vn30_domain::timestamp::MarketTimestamp;

    fn make_ts(delta_secs: i64) -> MarketTimestamp {
        MarketTimestamp::from_epoch_secs(1725300000 + delta_secs).expect("Timestamp hợp lệ")
    }

    #[test]
    fn test_symbol_state_initial_and_updates() {
        let mut state = SymbolState::new("HPG".to_string(), make_ts(0));
        assert_eq!(state.symbol, "HPG");
        assert_eq!(state.last_trade, None);
        assert_eq!(state.last_quote, None);
        assert_eq!(state.total_volume, 0.0);
        assert_eq!(state.updated_at, make_ts(0));

        // 1. Cập nhật Trade lần đầu
        let trade1 = Trade::new("HPG".to_string(), 28000.0, 5000.0, make_ts(1)).unwrap();
        assert!(state.update_trade(trade1.clone()).is_ok());
        assert_eq!(state.last_trade, Some(trade1));
        assert_eq!(state.total_volume, 5000.0);
        assert_eq!(state.updated_at, make_ts(1));

        // 2. Cập nhật Trade lần 2 (cộng dồn khối lượng)
        let trade2 = Trade::new("HPG".to_string(), 28100.0, 15000.0, make_ts(2)).unwrap();
        assert!(state.update_trade(trade2.clone()).is_ok());
        assert_eq!(state.last_trade, Some(trade2));
        assert_eq!(state.total_volume, 20000.0);
        assert_eq!(state.updated_at, make_ts(2));

        // 3. Cập nhật Quote
        let quote1 = Quote::new("HPG".to_string(), 28050.0, 1000.0, 28100.0, 2000.0, make_ts(3)).unwrap();
        assert!(state.update_quote(quote1.clone()).is_ok());
        assert_eq!(state.last_quote, Some(quote1));
        assert_eq!(state.updated_at, make_ts(3));
    }

    #[test]
    fn test_stale_trade_and_quote_rejected() {
        let mut state = SymbolState::new("VNM".to_string(), make_ts(10));

        let trade_new = Trade::new("VNM".to_string(), 65000.0, 100.0, make_ts(20)).unwrap();
        assert!(state.update_trade(trade_new.clone()).is_ok());

        // Lệnh Trade cũ hơn (timestamp 15 < 20) phải bị từ chối
        let trade_stale = Trade::new("VNM".to_string(), 64900.0, 50.0, make_ts(15)).unwrap();
        let res = state.update_trade(trade_stale);
        assert!(matches!(res, Err(PriceDataError::StaleData(_))));
        // Khối lượng và last_trade không bị thay đổi
        assert_eq!(state.total_volume, 100.0);
        assert_eq!(state.last_trade, Some(trade_new));

        // Tương tự với Quote
        let quote_new = Quote::new("VNM".to_string(), 64900.0, 100.0, 65000.0, 200.0, make_ts(25)).unwrap();
        assert!(state.update_quote(quote_new.clone()).is_ok());

        let quote_stale = Quote::new("VNM".to_string(), 64800.0, 100.0, 65000.0, 200.0, make_ts(22)).unwrap();
        let res_q = state.update_quote(quote_stale);
        assert!(matches!(res_q, Err(PriceDataError::StaleData(_))));
        assert_eq!(state.last_quote, Some(quote_new));
    }

    #[test]
    fn test_market_state_store_routing_and_getters() {
        let store = MarketStateStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        // Gửi Trade cho mã lowercase có khoảng trắng
        let trade = Trade::new("  fpt  ".to_string(), 125000.0, 1000.0, make_ts(1)).unwrap();
        store.update_event(&MarketEvent::Trade(trade));

        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        // Tra cứu với nhiều biến thể định dạng chuỗi
        assert_eq!(store.get_last_price("fpt"), Some(125000.0));
        assert_eq!(store.get_last_price(" FPT "), Some(125000.0));
        assert_eq!(store.get_last_price("UNKNOWN"), None);

        let state = store.get_state("fpt").expect("Phải tìm thấy FPT");
        assert_eq!(state.symbol, "FPT");
        assert_eq!(state.total_volume, 1000.0);
        assert_eq!(state.last_quote, None);

        // Gửi Quote cho FPT
        let quote = Quote::new("FPT".to_string(), 124500.0, 500.0, 125000.0, 800.0, make_ts(2)).unwrap();
        store.update_event(&MarketEvent::Quote(quote.clone()));

        let state_after_quote = store.get_state("FPT").unwrap();
        assert_eq!(state_after_quote.last_quote, Some(quote));
        assert_eq!(state_after_quote.total_volume, 1000.0); // quote không làm tăng total_volume
    }

    #[test]
    fn test_snapshot_all() {
        let store = MarketStateStore::new();
        let symbols = ["VIC", "VHM", "VRE"];

        for (idx, sym) in symbols.iter().enumerate() {
            let trade = Trade::new(sym.to_string(), 45000.0 + (idx as f64) * 1000.0, 100.0, make_ts(idx as i64)).unwrap();
            store.update_event(&MarketEvent::Trade(trade));
        }

        let snapshot = store.snapshot_all();
        assert_eq!(snapshot.len(), 3);

        let mut found_symbols: Vec<String> = snapshot.into_iter().map(|s| s.symbol).collect();
        found_symbols.sort();
        assert_eq!(found_symbols, vec!["VHM", "VIC", "VRE"]);
    }

    #[test]
    fn test_concurrent_access() {
        let store = Arc::new(MarketStateStore::new());
        let mut handles = Vec::new();

        // 5 luồng ghi liên tục cho các mã khác nhau
        for thread_id in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                let symbol = format!("SYM_{}", thread_id);
                for i in 1..=50 {
                    let trade = Trade::new(symbol.clone(), 10000.0 + (i as f64), 10.0, make_ts(i)).unwrap();
                    store_clone.update_event(&MarketEvent::Trade(trade));
                }
            });
            handles.push(handle);
        }

        // 5 luồng đọc song song
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for _ in 0..50 {
                    let _ = store_clone.get_last_price("SYM_0");
                    let _ = store_clone.snapshot_all();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Luồng chạy không được panic");
        }

        assert_eq!(store.len(), 5);
        for thread_id in 0..5 {
            let sym = format!("SYM_{}", thread_id);
            let state = store.get_state(&sym).expect("Mã phải tồn tại");
            assert_eq!(state.total_volume, 50.0 * 10.0);
            assert_eq!(store.get_last_price(&sym), Some(10050.0));
        }
    }
}

