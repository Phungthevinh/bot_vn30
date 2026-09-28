use std::collections::VecDeque;
use vn30_domain::market::{Candle, Timeframe, Trade};
use vn30_domain::timestamp::MarketTimestamp;

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
            let trade = make_trade("HPG", 28000.0 + (i as f64 * 100.0), 100.0, base_ts + (i * 60) + 10);
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
}
