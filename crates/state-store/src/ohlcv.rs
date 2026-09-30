use dashmap::DashMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use vn30_domain::market::{Candle, Timeframe, Trade};
use vn30_domain::timestamp::MarketTimestamp;

/// Cấu hình mặc định các khung thời gian và dung lượng lưu trữ nến trong phiên.
pub fn default_timeframe_config() -> Vec<(Timeframe, usize)> {
    vec![
        (Timeframe::M1, 512),
        (Timeframe::M15, 32),
        (Timeframe::H1, 8),
        (Timeframe::D1, 16),
    ]
}

/// Quản lý chuỗi nến (OHLCV) động của một mã chứng khoán tại một khung thời gian cụ thể.
///
/// Duy trì nến đang mở thời gian thực (`current`) và một bộ đệm vòng (Rolling Window)
/// chứa danh sách các nến đã đóng trong quá khứ (`closed`) với giới hạn dung lượng cố định (`capacity`).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolOhlcv {
    /// Giới hạn số lượng nến đã đóng tối đa được lưu trữ trong bộ nhớ RAM.
    pub capacity: usize,
    /// Cây nến đang mở (đang gom giao dịch khớp lệnh trong phiên hiện tại).
    pub current: Option<Candle>,
    /// Danh sách các cây nến đã đóng hoàn chỉnh trong quá khứ (FIFO buffer).
    pub closed: VecDeque<Candle>,
    /// Khung thời gian của chuỗi nến (ví dụ: `M1`, `M15`, `H1`, `D1`).
    pub timeframe: Timeframe,
}

/// Bộ lưu trữ và quản lý chuỗi nến thời gian thực (OHLCV) cho toàn bộ rổ cổ phiếu trên nhiều khung thời gian.
///
/// Sử dụng cấu trúc [`DashMap`] với cơ chế khóa phân mảnh (Sharded Locking) kết hợp [`HashMap`] nội bộ
/// để đảm bảo tính nguyên tử (Atomicity) theo từng mã chứng khoán khi cập nhật nhiều timeframe đồng thời.
#[derive(Debug)]
pub struct OhlcvStateStore {
    states: DashMap<String, HashMap<Timeframe, SymbolOhlcv>>,
    timeframe_config: Vec<(Timeframe, usize)>,
}

impl Default for OhlcvStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OhlcvStateStore {
    /// Khởi tạo Store với cấu hình khung thời gian mặc định.
    pub fn new() -> Self {
        Self::with_config(default_timeframe_config())
    }

    /// Khởi tạo Store với cấu hình khung thời gian tùy chỉnh.
    pub fn with_config(config: Vec<(Timeframe, usize)>) -> Self {
        Self {
            states: DashMap::new(),
            timeframe_config: config,
        }
    }

    /// Cập nhật một giao dịch khớp lệnh (`Trade`) vào tất cả các timeframe của mã chứng khoán tương ứng.
    ///
    /// # Luồng xử lý:
    /// 1. Chuẩn hóa symbol: `let symbol_key = trade.symbol.trim().to_uppercase();`
    /// 2. Dùng `self.states.entry(symbol_key).or_insert_with(...)`:
    ///    - Khởi tạo `HashMap<Timeframe, SymbolOhlcv>` mới nếu mã chưa tồn tại, dựa trên `self.timeframe_config`.
    /// 3. Duyệt qua các timeframe trong map:
    ///    - Gọi `ohlcv.update_trade(trade)`.
    ///    - Nếu có nến đóng (`Some(closed)`), gom vào danh sách `closed_candles`.
    /// 4. Trả về `closed_candles: Vec<Candle>`.
    pub fn update_trade(&self, trade: &Trade) -> Vec<Candle> {
        let symbol = trade.symbol.trim().to_uppercase();
        let mut map = self.states.entry(symbol).or_insert_with(|| {
            let mut map = HashMap::new();
            for (timeframe, capacity) in self.timeframe_config.iter() {
                map.insert(
                    timeframe.clone(),
                    SymbolOhlcv::new(timeframe.clone(), *capacity),
                );
            }
            map
        });
        let mut closed_candles = Vec::new();
        for (timeframe, _) in &self.timeframe_config {
            if let Some(ohlcv) = map.get_mut(timeframe) {
                if let Some(closed) = ohlcv.update_trade(trade) {
                    closed_candles.push(closed);
                }
            }
        }
        closed_candles
    }

    /// Lấy bản sao cây nến đang mở thời gian thực (`current`) của một mã tại một khung thời gian cụ thể.
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán (tự động cắt khoảng trắng và chuẩn hóa chữ hoa).
    /// - `timeframe`: Khung thời gian cần tra cứu (ví dụ: `Timeframe::M1`).
    ///
    /// # Giá trị trả về:
    /// - `Some(Candle)`: Cây nến đang mở và gom lệnh khớp trong phiên hiện tại.
    /// - `None`: Nếu mã chưa tồn tại hoặc chưa nhận được lệnh khớp nào.
    pub fn get_current_candle(&self, symbol: &str, timeframe: Timeframe) -> Option<Candle> {
        let symbol = symbol.trim().to_uppercase();
        let symbol_ohlcv = self.states.get(&symbol).and_then(|value| {
            value
                .get(&timeframe)
                .and_then(|ohlcv| ohlcv.current.clone())
        });

        return symbol_ohlcv;
    }

    /// Lấy bản sao danh sách tất cả các cây nến đã đóng hoàn chỉnh trong quá khứ (`closed`) theo thứ tự thời gian tăng dần (FIFO).
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán.
    /// - `timeframe`: Khung thời gian cần tra cứu.
    ///
    /// # Giá trị trả về:
    /// - `Some(Vec<Candle>)`: Danh sách nến đã đóng (giới hạn tối đa bởi dung lượng `capacity` của khung).
    /// - `None`: Nếu mã chưa có trong hệ thống.
    pub fn get_closed_candles(&self, symbol: &str, timeframe: Timeframe) -> Option<Vec<Candle>> {
        let symbol = symbol.trim().to_uppercase();
        let symbol_ohlcv = self.states.get(&symbol).and_then(|value| {
            value
                .get(&timeframe)
                .map(|ohlcv| ohlcv.closed.iter().cloned().collect())
        });
        return symbol_ohlcv;
    }

    /// Lấy bản sao của cây nến vừa đóng phiên gần nhất của một mã tại một khung thời gian cụ thể.
    ///
    /// Rất hữu ích cho các Indicator Engine hoặc Signal Engine chỉ cần kiểm tra mức giá đóng nến của phiên trước đó.
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán.
    /// - `timeframe`: Khung thời gian cần tra cứu.
    ///
    /// # Giá trị trả về:
    /// - `Some(Candle)`: Cây nến đã đóng gần nhất (`closed.back()`).
    /// - `None`: Nếu mã chưa có trong hệ thống hoặc chưa có cây nến nào đóng phiên.
    pub fn get_latest_closed_candle(&self, symbol: &str, timeframe: Timeframe) -> Option<Candle> {
        let symbol = symbol.trim().to_uppercase();
        let symbol_ohlcv = self.states.get(&symbol).and_then(|value| {
            value
                .get(&timeframe)
                .and_then(|ohlcv| ohlcv.closed.back().cloned())
        });
        return symbol_ohlcv;
    }

    /// Trả về tổng số lượng mã chứng khoán hiện đang được theo dõi và quản lý chuỗi nến trong Store.
    pub fn symbol_count(&self) -> usize {
        self.states.len()
    }

    /// Trả về danh sách tất cả các mã chứng khoán đang được theo dõi trong Store (đã được sắp xếp theo thứ tự A-Z).
    pub fn symbols(&self) -> Vec<String> {
        let mut symbols: Vec<String> = self.states.iter().map(|key| key.key().clone()).collect();
        symbols.sort();
        symbols
    }
}
impl SymbolOhlcv {
    /// Khởi tạo một khay lưu trữ nến mới cho một khung thời gian cụ thể.
    ///
    /// # Tham số:
    /// - `timeframe`: Khung thời gian gom nến.
    /// - `capacity`: Dung lượng tối đa của bộ đệm lịch sử nến (tự động đảm bảo tối thiểu là 1).
    pub fn new(timeframe: Timeframe, capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            capacity: cap,
            current: None,
            closed: VecDeque::with_capacity(cap),
            timeframe,
        }
    }

    /// Cập nhật một giao dịch khớp lệnh (`Trade`) vào chuỗi nến thời gian thực.
    ///
    /// # Cơ chế:
    /// - Nếu chưa có nến (`current` là `None`): Khởi tạo nến đầu tiên căn gióng theo `timeframe`, trả về `None`.
    /// - Nếu lệnh nằm trong phiên của nến hiện tại (`trade.timestamp < current.end_time`):
    ///   Cập nhật `high`, `low`, `close` và cộng dồn `volume`, trả về `None`.
    /// - Nếu lệnh bước sang phiên nến mới (`trade.timestamp >= current.end_time`):
    ///   Chốt nến cũ (`is_closed = true`), đẩy vào bộ đệm `closed` (cắt tỉa FIFO nếu vượt quá `capacity`),
    ///   khởi tạo nến mới cho phiên tiếp theo và trả về `Some(closed_candle)`.
    ///
    /// # Giá trị trả về:
    /// - `Some(Candle)`: Nếu có một cây nến vừa đóng phiên hoàn chỉnh.
    /// - `None`: Nếu nến hiện tại vẫn đang mở hoặc vừa khởi tạo.
    pub fn update_trade(&mut self, trade: &Trade) -> Option<Candle> {
        match self.current {
            Some(ref mut current) => {
                if trade.timestamp < current.end_time {
                    if trade.price > current.high {
                        current.high = trade.price;
                    }
                    if trade.price < current.low {
                        current.low = trade.price;
                    }
                    current.close = trade.price;
                    current.volume += trade.volume;
                    return None;
                } else {
                    current.is_closed = true;
                    self.closed.push_back(current.clone());
                    if self.closed.len() > self.capacity {
                        self.closed.pop_front();
                    }
                    let step = self.timeframe.duration_secs() as i64;
                    let start = (trade.timestamp.timestamp_secs() / step) * step;
                    let start_candle =
                        MarketTimestamp::from_epoch_secs(start).expect("Failed to create candle");
                    let end_candle = MarketTimestamp::from_epoch_secs(start + step)
                        .expect("Failed to create candle");
                    self.current = Some(
                        Candle::new(
                            trade.symbol.clone(),
                            trade.price,
                            trade.price,
                            trade.price,
                            trade.price,
                            trade.volume,
                            start_candle,
                            end_candle,
                        )
                        .expect("Failed to create candle"),
                    );
                    return self.closed.back().cloned();
                }
            }
            None => {
                let step = self.timeframe.duration_secs() as i64;
                let start = (trade.timestamp.timestamp_secs() / step) * step;
                let start_candle =
                    MarketTimestamp::from_epoch_secs(start).expect("Failed to create candle");
                let end_candle = MarketTimestamp::from_epoch_secs(start + step)
                    .expect("Failed to create candle");

                self.current = Some(
                    Candle::new(
                        trade.symbol.clone(),
                        trade.price,
                        trade.price,
                        trade.price,
                        trade.price,
                        trade.volume,
                        start_candle,
                        end_candle,
                    )
                    .expect("Failed to create candle"),
                );
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trade(symbol: &str, price: f64, volume: f64, epoch_secs: i64) -> Trade {
        let ts = MarketTimestamp::from_epoch_secs(epoch_secs).expect("Hợp lệ");
        Trade::new(symbol.to_string(), price, volume, ts).expect("Hợp lệ")
    }

    #[test]
    fn test_ohlcv_first_trade_initializes_current_candle() {
        let mut ohlcv = SymbolOhlcv::new(Timeframe::M1, 100);
        assert!(ohlcv.current.is_none());
        assert!(ohlcv.closed.is_empty());

        // Lệnh lúc 09:15:23 (epoch 1725300023)
        let trade = make_trade("HPG", 28000.0, 1000.0, 1725300023);
        let res = ohlcv.update_trade(&trade);

        // Nến mới mở, chưa có nến nào đóng
        assert!(res.is_none());
        assert!(ohlcv.closed.is_empty());

        let current = ohlcv.current.as_ref().unwrap();
        assert_eq!(current.symbol, "HPG");
        assert_eq!(current.open, 28000.0);
        assert_eq!(current.high, 28000.0);
        assert_eq!(current.low, 28000.0);
        assert_eq!(current.close, 28000.0);
        assert_eq!(current.volume, 1000.0);
        assert!(!current.is_closed);

        // Căn gióng đầu nến về đúng 09:15:00 và kết thúc 09:16:00
        assert_eq!(current.start_time.timestamp_secs(), 1725300000);
        assert_eq!(current.end_time.timestamp_secs(), 1725300060);
    }

    #[test]
    fn test_ohlcv_multiple_trades_same_minute() {
        let mut ohlcv = SymbolOhlcv::new(Timeframe::M1, 100);

        // Lệnh 1 lúc 09:15:05 giá 28.0, vol 1000
        ohlcv.update_trade(&make_trade("HPG", 28000.0, 1000.0, 1725300005));

        // Lệnh 2 lúc 09:15:20 giá 28.5 (đỉnh mới), vol 2000
        let res2 = ohlcv.update_trade(&make_trade("HPG", 28500.0, 2000.0, 1725300020));
        assert!(res2.is_none());

        // Lệnh 3 lúc 09:15:45 giá 27.5 (đáy mới), vol 500
        let res3 = ohlcv.update_trade(&make_trade("HPG", 27500.0, 500.0, 1725300045));
        assert!(res3.is_none());

        // Lệnh 4 lúc 09:15:59 giá 28.2 (đóng cửa), vol 1500
        let res4 = ohlcv.update_trade(&make_trade("HPG", 28200.0, 1500.0, 1725300059));
        assert!(res4.is_none());

        // Toàn bộ lệnh trong phút 15 chỉ cập nhật cây nến đó
        assert!(ohlcv.closed.is_empty());
        let current = ohlcv.current.as_ref().unwrap();
        assert_eq!(current.open, 28000.0);
        assert_eq!(current.high, 28500.0);
        assert_eq!(current.low, 27500.0);
        assert_eq!(current.close, 28200.0);
        assert_eq!(current.volume, 5000.0); // 1000 + 2000 + 500 + 1500
    }

    #[test]
    fn test_ohlcv_new_minute_closes_old_candle_and_returns_it() {
        let mut ohlcv = SymbolOhlcv::new(Timeframe::M1, 100);

        // Lệnh ở phút 15 (1725300010)
        ohlcv.update_trade(&make_trade("HPG", 28000.0, 1000.0, 1725300010));
        ohlcv.update_trade(&make_trade("HPG", 28500.0, 1500.0, 1725300040));

        // Lệnh bước sang phút 16 (1725300065 = 09:16:05)
        let closed_res = ohlcv.update_trade(&make_trade("HPG", 28800.0, 2000.0, 1725300065));

        // Phải trả về cây nến phút 15 vừa đóng
        assert!(closed_res.is_some());
        let closed_candle = closed_res.unwrap();
        assert_eq!(closed_candle.symbol, "HPG");
        assert_eq!(closed_candle.open, 28000.0);
        assert_eq!(closed_candle.high, 28500.0);
        assert_eq!(closed_candle.low, 28000.0);
        assert_eq!(closed_candle.close, 28500.0);
        assert_eq!(closed_candle.volume, 2500.0);
        assert!(closed_candle.is_closed);
        assert_eq!(closed_candle.start_time.timestamp_secs(), 1725300000);
        assert_eq!(closed_candle.end_time.timestamp_secs(), 1725300060);

        // Cây nến cũ đã được lưu vào kho `closed`
        assert_eq!(ohlcv.closed.len(), 1);
        assert_eq!(ohlcv.closed[0], closed_candle);

        // Cây nến mới trong `current` thuộc về phút 16
        let current = ohlcv.current.as_ref().unwrap();
        assert_eq!(current.open, 28800.0);
        assert_eq!(current.close, 28800.0);
        assert_eq!(current.volume, 2000.0);
        assert!(!current.is_closed);
        assert_eq!(current.start_time.timestamp_secs(), 1725300060);
        assert_eq!(current.end_time.timestamp_secs(), 1725300120);
    }

    #[test]
    fn test_ohlcv_bounded_capacity_fifo_eviction() {
        // Khởi tạo giới hạn tối đa chỉ lưu 3 cây nến
        let mut ohlcv = SymbolOhlcv::new(Timeframe::M1, 3);
        let base_ts = 1725300000; // 09:15:00

        // Gửi lệnh cho 5 phút liên tiếp (phút 0, 1, 2, 3, 4)
        for i in 0..5 {
            let trade = make_trade(
                "HPG",
                28000.0 + (i as f64 * 100.0),
                100.0,
                base_ts + (i * 60) + 10,
            );
            ohlcv.update_trade(&trade);
        }

        // Sau 5 nến, nến đang mở là nến thứ 5 (phút 4),
        // và kho nến đóng `closed` chỉ được phép chứa tối đa 3 cây nến
        assert_eq!(ohlcv.closed.len(), 3);

        // 3 cây nến còn lại trong kho phải là các nến của phút 1, 2, 3 (nến phút 0 đã bị FIFO pop)
        assert_eq!(ohlcv.closed[0].start_time.timestamp_secs(), base_ts + 60);
        assert_eq!(ohlcv.closed[1].start_time.timestamp_secs(), base_ts + 120);
        assert_eq!(ohlcv.closed[2].start_time.timestamp_secs(), base_ts + 180);
    }

    #[test]
    fn test_ohlcv_timeframe_m15() {
        let mut ohlcv = SymbolOhlcv::new(Timeframe::M15, 10);
        let base_ts = 1725300000; // 09:15:00

        // Lệnh lúc 09:20:00 (vẫn trong khoảng 09:15:00 -> 09:30:00)
        let res1 = ohlcv.update_trade(&make_trade("HPG", 28000.0, 100.0, base_ts + 300));
        assert!(res1.is_none());

        // Lệnh lúc 09:29:59 (vẫn trong khoảng 09:15:00 -> 09:30:00)
        let res2 = ohlcv.update_trade(&make_trade("HPG", 28500.0, 200.0, base_ts + 899));
        assert!(res2.is_none());
        assert_eq!(ohlcv.closed.len(), 0);

        // Lệnh lúc 09:30:01 (bước sang nến 15 phút tiếp theo)
        let res3 = ohlcv.update_trade(&make_trade("HPG", 28600.0, 50.0, base_ts + 901));
        assert!(res3.is_some());
        let closed = res3.unwrap();
        assert_eq!(closed.start_time.timestamp_secs(), base_ts);
        assert_eq!(closed.end_time.timestamp_secs(), base_ts + 900);
        assert_eq!(ohlcv.closed.len(), 1);
    }

    #[test]
    fn test_ohlcv_state_store_single_trade_initializes_all_timeframes() {
        let store = OhlcvStateStore::new();
        assert_eq!(store.symbol_count(), 0);
        assert!(store.symbols().is_empty());

        // Lệnh đầu tiên của HPG lúc 09:15:23
        let trade = make_trade("hpg", 28000.0, 1000.0, 1725300023);
        let closed = store.update_trade(&trade);

        // Nến mới mở, chưa có nến nào đóng
        assert!(closed.is_empty());
        assert_eq!(store.symbol_count(), 1);
        assert_eq!(store.symbols(), vec!["HPG"]);

        // Kiểm tra tất cả các khung thời gian đều được tự động khởi tạo nến current
        for tf in [Timeframe::M1, Timeframe::M15, Timeframe::H1, Timeframe::D1] {
            let current = store.get_current_candle("HPG", tf);
            assert!(current.is_some());
            let c = current.unwrap();
            assert_eq!(c.symbol, "HPG");
            assert_eq!(c.open, 28000.0);
            assert_eq!(c.close, 28000.0);
            assert_eq!(c.volume, 1000.0);
            assert!(!c.is_closed);

            // Chưa có nến nào đóng
            let closed_list = store.get_closed_candles("HPG", tf);
            assert_eq!(closed_list, Some(vec![]));
            assert!(store.get_latest_closed_candle("HPG", tf).is_none());
        }

        // Mã không tồn tại trả về None
        assert!(store.get_current_candle("VIC", Timeframe::M1).is_none());
        assert!(store.get_closed_candles("VIC", Timeframe::M1).is_none());
        assert!(store.get_latest_closed_candle("VIC", Timeframe::M1).is_none());
    }

    #[test]
    fn test_ohlcv_state_store_multi_timeframe_closes() {
        let store = OhlcvStateStore::new();
        let base_ts = 1725300000; // 09:15:00

        // 1. Lệnh tại phút 15 (09:15:30)
        let c0 = store.update_trade(&make_trade("HPG", 28000.0, 1000.0, base_ts + 30));
        assert!(c0.is_empty());

        // 2. Lệnh bước sang phút 16 (09:16:05 = base + 65s)
        // -> Chỉ có nến M1 đóng (09:15:00 -> 09:16:00), nến M15 vẫn đang mở
        let c1 = store.update_trade(&make_trade("HPG", 28200.0, 1500.0, base_ts + 65));
        assert_eq!(c1.len(), 1);
        assert_eq!(c1[0].start_time.timestamp_secs(), base_ts);
        assert_eq!(c1[0].end_time.timestamp_secs(), base_ts + 60);
        assert_eq!(c1[0].close, 28000.0);
        assert_eq!(c1[0].volume, 1000.0);

        // 3. Lệnh bước sang mốc 15 phút tiếp theo (09:30:05 = base + 905s)
        // -> Đồng thời đóng cả nến M1 (phút 16) VÀ nến M15 (09:15 -> 09:30)
        let c2 = store.update_trade(&make_trade("HPG", 28500.0, 2000.0, base_ts + 905));
        assert_eq!(c2.len(), 2);

        // Nhờ cơ chế duyệt theo timeframe_config: Nến M1 luôn đứng trước, M15 đứng sau
        let closed_m1 = &c2[0];
        assert_eq!(closed_m1.start_time.timestamp_secs(), base_ts + 60);
        assert_eq!(closed_m1.end_time.timestamp_secs(), base_ts + 120);

        let closed_m15 = &c2[1];
        assert_eq!(closed_m15.start_time.timestamp_secs(), base_ts);
        assert_eq!(closed_m15.end_time.timestamp_secs(), base_ts + 900);
        assert_eq!(closed_m15.volume, 2500.0); // 1000 + 1500

        // Kiểm tra tra cứu nến đóng gần nhất
        let latest_m15 = store.get_latest_closed_candle("HPG", Timeframe::M15);
        assert_eq!(latest_m15, Some(closed_m15.clone()));

        // Kiểm tra danh sách nến đóng M1 có 2 cây nến
        let closed_m1_list = store.get_closed_candles("HPG", Timeframe::M1).unwrap();
        assert_eq!(closed_m1_list.len(), 2);
    }

    #[test]
    fn test_ohlcv_state_store_custom_capacity_and_fifo_eviction() {
        // Cấu hình dung lượng nhỏ: M1 tối đa 2 nến
        let config = vec![(Timeframe::M1, 2)];
        let store = OhlcvStateStore::with_config(config);
        let base_ts = 1725300000; // 09:15:00

        // Gửi 4 nến qua 4 phút (09:15, 09:16, 09:17, 09:18)
        for i in 0..4 {
            store.update_trade(&make_trade("VNM", 70000.0, 100.0, base_ts + (i * 60) + 10));
        }

        // Đẩy thêm 1 lệnh ở phút thứ 5 để chốt nến thứ 4
        store.update_trade(&make_trade("VNM", 70500.0, 100.0, base_ts + (4 * 60) + 10));

        let closed_vnm = store.get_closed_candles("VNM", Timeframe::M1).unwrap();
        // Dung lượng tối đa là 2 nến, nên chỉ giữ lại nến phút 2 và phút 3
        assert_eq!(closed_vnm.len(), 2);
        assert_eq!(closed_vnm[0].start_time.timestamp_secs(), base_ts + 120);
        assert_eq!(closed_vnm[1].start_time.timestamp_secs(), base_ts + 180);

        let latest = store.get_latest_closed_candle("VNM", Timeframe::M1).unwrap();
        assert_eq!(latest, closed_vnm[1]);
    }

    #[test]
    fn test_ohlcv_state_store_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(OhlcvStateStore::new());
        let symbols = vec!["HPG", "FPT", "VNM", "VIC", "TCB"];
        let mut handles = Vec::new();

        // 5 luồng đồng thời cập nhật dữ liệu cho 5 mã cổ phiếu khác nhau
        for sym in symbols.clone() {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                let base_ts = 1725300000;
                for i in 0..60 {
                    let trade = make_trade(sym, 50000.0 + (i as f64), 10.0, base_ts + i);
                    store_clone.update_trade(&trade);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Luồng chạy không được panic");
        }

        // Toàn bộ 5 mã đều được khởi tạo toàn vẹn, không bị race condition
        assert_eq!(store.symbol_count(), 5);
        assert_eq!(store.symbols(), vec!["FPT", "HPG", "TCB", "VIC", "VNM"]);

        for sym in symbols {
            let current = store.get_current_candle(sym, Timeframe::M1);
            assert!(current.is_some());
            assert_eq!(current.unwrap().volume, 600.0); // 60 lệnh * 10 vol
        }
    }
}
