use std::collections::{hash_map, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use vn30_domain::market::{MarketEvent, Quote};

#[derive(Debug)]
pub struct EventDeduplicator {
    capacity: usize,
    dedup_trades: bool,
    seen_quotes: HashSet<u64>,
    order_quotes: VecDeque<u64>,

    // Metrics phục vụ Observability
    total_received: u64,
    dropped_quotes: u64,
    dropped_trades: u64,
}

impl EventDeduplicator {
    /// Khởi tạo bộ deduplicator với giới hạn dung lượng (capacity) và cờ dedup_trades
    pub fn new(capacity: usize, dedup_trades: bool) -> Self {
        let cap = if capacity == 0 { 10_000 } else { capacity };
        Self {
            capacity: cap,
            dedup_trades,
            seen_quotes: HashSet::with_capacity(cap),
            order_quotes: VecDeque::with_capacity(cap),
            total_received: 0,
            dropped_quotes: 0,
            dropped_trades: 0,
        }
    }

    // hàm tính fingerprint mã băm duy nhất
    pub fn fingerprint(quote: &Quote) -> u64 {
        let mut hasher = hash_map::DefaultHasher::new();
        quote.symbol.hash(&mut hasher);
        quote.bid_price.to_bits().hash(&mut hasher);
        quote.bid_vol.to_bits().hash(&mut hasher);
        quote.ask_price.to_bits().hash(&mut hasher);
        quote.ask_vol.to_bits().hash(&mut hasher);
        quote.timestamp.hash(&mut hasher);
        return hasher.finish();
    }

    //kiểm tra trùng lặp và đẩy phần tử cũ ra ngoài khi vượt quá capacity
    pub fn is_duplicate(&mut self, event: &MarketEvent) -> bool {
        self.total_received += 1;

        match event {
            MarketEvent::Quote(quote) => {
                let fb = Self::fingerprint(quote);
                if self.seen_quotes.contains(&fb) {
                    self.dropped_quotes += 1;
                    return true;
                } else {
                    self.seen_quotes.insert(fb);
                    self.order_quotes.push_back(fb);
                    //kiểm tra và loại bỏ phần tử cũ
                    while self.order_quotes.len() > self.capacity {
                        if let Some(old_fb) = self.order_quotes.pop_front() {
                            self.seen_quotes.remove(&old_fb);
                        }
                    }
                    return false;
                }
            }
            MarketEvent::Trade(_) => {
                return false;
            }
        }
    }

    /// Trả về capacity tối đa được cấu hình
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Trả về cờ cấu hình dedup cho trade
    pub fn dedup_trades(&self) -> bool {
        self.dedup_trades
    }

    /// Tổng số bản tin sự kiện đã tiếp nhận
    pub fn total_received(&self) -> u64 {
        self.total_received
    }

    /// Số lượng quote trùng lặp đã bị loại bỏ
    pub fn dropped_quotes(&self) -> u64 {
        self.dropped_quotes
    }

    /// Số lượng trade trùng lặp đã bị loại bỏ
    pub fn dropped_trades(&self) -> u64 {
        self.dropped_trades
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vn30_domain::market::Trade;
    use vn30_domain::timestamp::MarketTimestamp;

    fn make_timestamp(secs: i64) -> MarketTimestamp {
        MarketTimestamp::from_epoch_secs(secs).expect("Timestamp hợp lệ")
    }

    fn make_sample_quote(symbol: &str, bid_price: f64, ask_price: f64, secs: i64) -> Quote {
        Quote::new(
            symbol.to_string(),
            bid_price,
            100.0,
            ask_price,
            200.0,
            make_timestamp(secs),
        )
        .expect("Quote hợp lệ")
    }

    fn make_sample_trade(symbol: &str, price: f64, volume: f64, secs: i64) -> Trade {
        Trade::new(
            symbol.to_string(),
            price,
            volume,
            make_timestamp(secs),
        )
        .expect("Trade hợp lệ")
    }

    #[test]
    fn test_zero_capacity_fallback_to_default() {
        let dedup = EventDeduplicator::new(0, false);
        assert_eq!(dedup.capacity(), 10_000);
        assert!(!dedup.dedup_trades());
        assert_eq!(dedup.total_received(), 0);
        assert_eq!(dedup.dropped_quotes(), 0);
        assert_eq!(dedup.dropped_trades(), 0);
    }

    #[test]
    fn test_quote_duplicate_detected() {
        let mut dedup = EventDeduplicator::new(100, false);
        let quote = make_sample_quote("HPG", 28000.0, 28100.0, 1726000000);
        let event = MarketEvent::Quote(quote);

        // Lần 1: bản tin mới -> không trùng
        assert!(!dedup.is_duplicate(&event));
        assert_eq!(dedup.total_received(), 1);
        assert_eq!(dedup.dropped_quotes(), 0);

        // Lần 2: bản tin giống hệt -> phát hiện trùng lặp
        assert!(dedup.is_duplicate(&event));
        assert_eq!(dedup.total_received(), 2);
        assert_eq!(dedup.dropped_quotes(), 1);
    }

    #[test]
    fn test_quote_different_fields_not_duplicate() {
        let mut dedup = EventDeduplicator::new(100, false);
        let q1 = MarketEvent::Quote(make_sample_quote("HPG", 28000.0, 28100.0, 1726000000));
        let q2 = MarketEvent::Quote(make_sample_quote("HPG", 28050.0, 28100.0, 1726000000)); // khác bid_price
        let q3 = MarketEvent::Quote(make_sample_quote("HPG", 28000.0, 28100.0, 1726000001)); // khác timestamp
        let q4 = MarketEvent::Quote(make_sample_quote("VNM", 28000.0, 28100.0, 1726000000)); // khác mã symbol

        assert!(!dedup.is_duplicate(&q1));
        assert!(!dedup.is_duplicate(&q2));
        assert!(!dedup.is_duplicate(&q3));
        assert!(!dedup.is_duplicate(&q4));
        assert_eq!(dedup.total_received(), 4);
        assert_eq!(dedup.dropped_quotes(), 0);
    }

    #[test]
    fn test_capacity_eviction_lru_fifo() {
        // Khởi tạo capacity nhỏ = 2
        let mut dedup = EventDeduplicator::new(2, false);

        let q1 = MarketEvent::Quote(make_sample_quote("HPG", 28000.0, 28100.0, 1726000001));
        let q2 = MarketEvent::Quote(make_sample_quote("HPG", 28000.0, 28100.0, 1726000002));
        let q3 = MarketEvent::Quote(make_sample_quote("HPG", 28000.0, 28100.0, 1726000003));

        // Nạp Q1 và Q2 -> bộ đệm đầy (capacity = 2)
        assert!(!dedup.is_duplicate(&q1));
        assert!(!dedup.is_duplicate(&q2));

        // Nạp Q3 -> Q1 là bản tin cũ nhất sẽ bị loại bỏ (evicted)
        assert!(!dedup.is_duplicate(&q3));

        // Gửi lại Q3 -> vẫn còn trong cache -> phát hiện trùng lặp
        assert!(dedup.is_duplicate(&q3));

        // Gửi lại Q1 -> vì đã bị evict ra khỏi bộ đệm nên được coi là bản tin mới
        assert!(!dedup.is_duplicate(&q1));
    }

    #[test]
    fn test_trade_never_duplicated() {
        let mut dedup = EventDeduplicator::new(100, false);
        let trade = make_sample_trade("HPG", 28000.0, 500.0, 1726000000);
        let event = MarketEvent::Trade(trade);

        // Gửi 2 trade giống hệt nhau liên tiếp -> không bị coi là trùng lặp
        assert!(!dedup.is_duplicate(&event));
        assert!(!dedup.is_duplicate(&event));
        assert_eq!(dedup.total_received(), 2);
        assert_eq!(dedup.dropped_trades(), 0);
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let q1 = make_sample_quote("HPG", 28000.0, 28100.0, 1726000000);
        let q2 = make_sample_quote("HPG", 28000.0, 28100.0, 1726000000);
        assert_eq!(EventDeduplicator::fingerprint(&q1), EventDeduplicator::fingerprint(&q2));
    }
}
