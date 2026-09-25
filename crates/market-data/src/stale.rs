use std::collections::HashMap;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use vn30_domain::market::MarketEvent;

/// Bộ phát hiện dữ liệu thị trường bị đóng băng hoặc quá hạn (Stale Data Detector) cho từng mã chứng khoán.
///
/// Sử dụng đồng hồ đơn điệu (`Instant`) để tính toán khoảng thời gian trôi qua kể từ lần cập nhật gần nhất,
/// tránh bị ảnh hưởng bởi hiện tượng nhảy ngược giờ hệ thống (NTP clock drift).
#[derive(Debug)]
pub struct StaleDataDetector {
    /// Ngưỡng thời gian tối đa cho phép không có bản tin mới trước khi bị coi là `Stale`.
    threshold: Duration,
    /// Thời điểm cập nhật gần nhất của từng mã
    last_update: HashMap<String, Instant>,
    /// Danh sách các mã cổ phiếu trong rổ cần theo dõi (ví dụ: rổ VN30)
    watched_symbols: HashSet<String>,
}

/// Trạng thái sống/chết (Liveness) của một mã chứng khoán trong luồng phân tích thời gian thực.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLiveness {
    /// Đang nhận dữ liệu bình thường trong hạn threshold
    Active,
    /// Quá hạn threshold chưa có bản tin mới (đang bị treo/mất kết nối)
    Stale,
    /// Nằm trong danh sách theo dõi nhưng từ lúc bật bot chưa từng nhận được bản tin nào
    NeverSeen,
    /// Mã không nằm trong danh sách theo dõi (không thuộc rổ VN30)
    Untracked,
}

impl StaleDataDetector {
    /// Khởi tạo bộ phát hiện dữ liệu quá hạn (`StaleDataDetector`).
    ///
    /// # Tham số:
    /// - `threshold`: Ngưỡng thời gian tối đa cho phép không có bản tin mới trước khi bị coi là `Stale`.
    /// - `symbols`: Danh sách các mã cổ phiếu cần theo dõi ban đầu (ví dụ: rổ 30 mã VN30).
    ///
    /// # Xử lý:
    /// Tự động làm sạch khoảng trắng, viết hoa mã chứng khoán và loại bỏ các chuỗi rỗng.
    pub fn new<S, D>(threshold: D, symbols: S) -> Self
    where
        S: Into<Vec<String>>,
        D: Into<Duration>,
    {
        Self {
            last_update: HashMap::new(),
            threshold: threshold.into(),
            watched_symbols: symbols
                .into()
                .into_iter()
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    /// Ghi nhận mốc thời gian nhận bản tin mới nhất cho một mã chứng khoán.
    ///
    /// # Tham số:
    /// - `symbol`: Tên mã nhận được sự kiện (ví dụ: `"HPG"`, `"VIC"`).
    /// - `now`: Mốc thời gian nhận tin (đo bằng `Instant` của bot).
    ///
    /// # Giá trị trả về:
    /// - `true`: Mã này nằm trong danh sách theo dõi (`watched_symbols`) và đã được cập nhật mốc thời gian mới.
    /// - `false`: Mã này không nằm trong danh sách theo dõi và bị bỏ qua.
    pub fn record_update(&mut self, symbol: &str, now: Instant) -> bool {
        let symbol = symbol.trim().to_uppercase();
        if self.watched_symbols.contains(&symbol) {
            self.last_update.insert(symbol, now);
            true
        } else {
            false
        }
    }

    /// Kiểm tra trạng thái sống/chết (Liveness) của một mã chứng khoán tại thời điểm `now`.
    ///
    /// # Tham số:
    /// - `symbol`: Tên mã cần kiểm tra (ví dụ: `"HPG"`, `"VIC"`).
    /// - `now`: Mốc thời gian hiện tại dùng để đối chiếu (đo bằng `Instant`).
    ///
    /// # Giá trị trả về:
    /// - [`SymbolLiveness::Untracked`]: Mã không nằm trong danh sách theo dõi (`watched_symbols`).
    /// - [`SymbolLiveness::NeverSeen`]: Mã nằm trong danh sách theo dõi nhưng chưa từng nhận được bản tin nào kể từ khi khởi động.
    /// - [`SymbolLiveness::Stale`]: Mã đã quá hạn `threshold` mà không nhận thêm bản tin mới.
    /// - [`SymbolLiveness::Active`]: Mã vẫn đang nhận dữ liệu bình thường trong hạn `threshold`.
    pub fn check_status(&self, symbol: &str, now: Instant) -> SymbolLiveness {
        let symbol = symbol.trim().to_uppercase();
        if !self.watched_symbols.contains(&symbol) {
            return SymbolLiveness::Untracked;
        } else {
            match self.last_update.get(&symbol) {
                Some(&last_time) => {
                    let elapsed = now.duration_since(last_time);
                    if elapsed >= self.threshold {
                        return SymbolLiveness::Stale;
                    } else {
                        return SymbolLiveness::Active;
                    }
                }
                None => {
                    return SymbolLiveness::NeverSeen;
                }
            }
        }
    }

    /// Quét và lấy danh sách toàn bộ các mã đang gặp sự cố dữ liệu (Stale hoặc NeverSeen) tại thời điểm `now`.
    ///
    /// Phương thức này phục vụ cho tầng Observability / Metrics / Health Monitor định kỳ kiểm tra.
    ///
    /// # Tham số:
    /// - `now`: Mốc thời gian đối chiếu (đo bằng `Instant`).
    ///
    /// # Giá trị trả về:
    /// - `Vec<String>`: Danh sách các mã cổ phiếu đang bị đóng băng hoặc chưa từng nhận được dữ liệu.
    pub fn get_stale_symbols(&self, now: Instant) -> Vec<String> {
        let mut new_arr: Vec<String> = Vec::new();

        for symbol in &self.watched_symbols {
            match self.check_status(symbol, now) {
                SymbolLiveness::Stale => new_arr.push(symbol.clone()),
                SymbolLiveness::NeverSeen => new_arr.push(symbol.clone()),
                _ => {}
            }
        }

        return new_arr;
    }

    /// Ghi nhận sự kiện thị trường chuẩn hóa ([`MarketEvent`]) vào hệ thống.
    ///
    /// Phương thức tiện ích này tự động trích xuất mã chứng khoán từ [`MarketEvent::Trade`]
    /// hoặc [`MarketEvent::Quote`] và chuyển tiếp tới [`Self::record_update`].
    ///
    /// # Tham số:
    /// - `event`: Bản tin sự kiện thị trường chuẩn hóa vừa nhận được.
    /// - `now`: Mốc thời gian nhận tin (đo bằng `Instant`).
    ///
    /// # Giá trị trả về:
    /// - `true`: Mã trong sự kiện nằm trong danh sách theo dõi và được cập nhật thành công.
    /// - `false`: Mã không nằm trong danh sách theo dõi và bị bỏ qua.
    pub fn record_event(&mut self, event: &MarketEvent, now: Instant) -> bool {
        match event {
            MarketEvent::Trade(trade) => self.record_update(&trade.symbol, now),
            MarketEvent::Quote(quote) => self.record_update(&quote.symbol, now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vn30_domain::market::{Quote, Trade};
    use vn30_domain::timestamp::MarketTimestamp;

    fn dummy_ts() -> MarketTimestamp {
        MarketTimestamp::from_epoch_secs(1725300000).expect("Timestamp hợp lệ")
    }

    #[test]
    fn test_init_and_normalization() {
        let detector = StaleDataDetector::new(
            Duration::from_secs(10),
            vec![
                "  hpg ".to_string(),
                "VIC".to_string(),
                "   ".to_string(),
                "".to_string(),
            ],
        );

        let now = Instant::now();
        // HPG đã được chuẩn hóa viết hoa và cắt khoảng trắng
        assert_eq!(
            detector.check_status("HPG", now),
            SymbolLiveness::NeverSeen
        );
        assert_eq!(
            detector.check_status("hpg", now),
            SymbolLiveness::NeverSeen
        );
        assert_eq!(
            detector.check_status("VIC", now),
            SymbolLiveness::NeverSeen
        );
        // Mã rỗng đã bị lọc bỏ
        assert_eq!(detector.check_status("", now), SymbolLiveness::Untracked);
    }

    #[test]
    fn test_untracked_symbol() {
        let detector =
            StaleDataDetector::new(Duration::from_secs(10), vec!["HPG".to_string()]);
        let now = Instant::now();
        assert_eq!(
            detector.check_status("VNM", now),
            SymbolLiveness::Untracked
        );
    }

    #[test]
    fn test_active_and_stale_transition() {
        let mut detector = StaleDataDetector::new(
            Duration::from_secs(10),
            vec!["HPG".to_string(), "VIC".to_string()],
        );

        let t0 = Instant::now();
        // Cập nhật HPG tại t0
        assert!(detector.record_update("HPG", t0));

        // Tại t0 + 5s (trong hạn 10s) -> HPG là Active
        let t1 = t0 + Duration::from_secs(5);
        assert_eq!(detector.check_status("HPG", t1), SymbolLiveness::Active);
        // VIC chưa nhận tin -> NeverSeen
        assert_eq!(
            detector.check_status("VIC", t1),
            SymbolLiveness::NeverSeen
        );

        // Tại t0 + 10s (chạm ngưỡng) -> HPG chuyển sang Stale
        let t2 = t0 + Duration::from_secs(10);
        assert_eq!(detector.check_status("HPG", t2), SymbolLiveness::Stale);

        // Tại t0 + 20s (vượt xa ngưỡng) -> HPG vẫn là Stale
        let t3 = t0 + Duration::from_secs(20);
        assert_eq!(detector.check_status("HPG", t3), SymbolLiveness::Stale);
    }

    #[test]
    fn test_recovery_from_stale() {
        let mut detector =
            StaleDataDetector::new(Duration::from_secs(10), vec!["HPG".to_string()]);

        let t0 = Instant::now();
        detector.record_update("HPG", t0);

        // Lúc t0 + 15s: HPG bị Stale
        let t1 = t0 + Duration::from_secs(15);
        assert_eq!(detector.check_status("HPG", t1), SymbolLiveness::Stale);

        // Nhận tin mới tại t1
        detector.record_update("HPG", t1);

        // Tại t1 + 2s: HPG hồi phục lại Active
        let t2 = t1 + Duration::from_secs(2);
        assert_eq!(detector.check_status("HPG", t2), SymbolLiveness::Active);
    }

    #[test]
    fn test_record_market_event_trade_and_quote() {
        let mut detector = StaleDataDetector::new(
            Duration::from_secs(10),
            vec!["HPG".to_string(), "VIC".to_string()],
        );

        let now = Instant::now();

        // 1. Ghi nhận Trade cho HPG
        let trade = Trade::new("HPG".to_string(), 28000.0, 1000.0, dummy_ts()).unwrap();
        let event_trade = MarketEvent::Trade(trade);
        assert!(detector.record_event(&event_trade, now));
        assert_eq!(detector.check_status("HPG", now), SymbolLiveness::Active);

        // 2. Ghi nhận Quote cho VIC
        let quote = Quote::new(
            "VIC".to_string(),
            45000.0,
            100.0,
            45100.0,
            200.0,
            dummy_ts(),
        )
        .unwrap();
        let event_quote = MarketEvent::Quote(quote);
        assert!(detector.record_event(&event_quote, now));
        assert_eq!(detector.check_status("VIC", now), SymbolLiveness::Active);

        // 3. Sự kiện với mã không theo dõi -> trả về false
        let untracked_trade =
            Trade::new("XYZ".to_string(), 10000.0, 50.0, dummy_ts()).unwrap();
        assert!(!detector.record_event(&MarketEvent::Trade(untracked_trade), now));
    }

    #[test]
    fn test_get_stale_symbols_bulk_scan() {
        let mut detector = StaleDataDetector::new(
            Duration::from_secs(10),
            vec!["HPG".to_string(), "VIC".to_string(), "VNM".to_string()],
        );

        let t0 = Instant::now();

        // Ban đầu chưa có tin nào -> Cả 3 đều NeverSeen -> get_stale_symbols gom cả 3
        let initial_stale = detector.get_stale_symbols(t0);
        assert_eq!(initial_stale.len(), 3);

        // Cập nhật HPG và VIC tại t0
        detector.record_update("HPG", t0);
        detector.record_update("VIC", t0);

        // Tại t0 + 2s: HPG và VIC Active, chỉ còn VNM là NeverSeen
        let t1 = t0 + Duration::from_secs(2);
        let stale_t1 = detector.get_stale_symbols(t1);
        assert_eq!(stale_t1.len(), 1);
        assert!(stale_t1.contains(&"VNM".to_string()));

        // Tại t0 + 12s: HPG và VIC vượt 10s trở thành Stale, VNM vẫn NeverSeen -> gom cả 3
        let t2 = t0 + Duration::from_secs(12);
        let stale_t2 = detector.get_stale_symbols(t2);
        assert_eq!(stale_t2.len(), 3);
    }
}

