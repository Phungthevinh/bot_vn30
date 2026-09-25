use serde::{Deserialize, Serialize};

use crate::errors::MarketDataError;
use crate::timestamp::MarketTimestamp;

/// Cấu trúc dữ liệu biểu diễn một giao dịch khớp lệnh (Trade) thực tế trên thị trường.
///
/// Mỗi bản tin khớp lệnh mang tính chất duy nhất theo thời gian và không bao giờ bị loại bỏ trùng lặp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    /// Mã chứng khoán chuẩn hóa viết hoa (ví dụ: `"HPG"`, `"VN30F2409"`).
    pub symbol: String,
    /// Mức giá khớp lệnh (phải là số hữu hạn và lớn hơn 0).
    pub price: f64,
    /// Khối lượng khớp lệnh (phải là số hữu hạn và lớn hơn 0).
    pub volume: f64,
    /// Mốc thời gian chuẩn hóa khi giao dịch phát sinh.
    pub timestamp: MarketTimestamp,
}

/// Cấu trúc dữ liệu biểu diễn sổ lệnh / mức giá chào mua và chào bán tốt nhất (BBO - Best Bid/Offer).
///
/// Các bản tin Quote cùng giá, cùng khối lượng và cùng timestamp có thể bị lọc trùng (deduplicated).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    /// Mã chứng khoán chuẩn hóa viết hoa.
    pub symbol: String,
    /// Giá đặt mua tốt nhất (Best Bid Price).
    pub bid_price: f64,
    /// Khối lượng đặt mua tương ứng.
    pub bid_vol: f64,
    /// Giá đặt bán tốt nhất (Best Ask Price).
    pub ask_price: f64,
    /// Khối lượng đặt bán tương ứng.
    pub ask_vol: f64,
    /// Mốc thời gian chuẩn hóa của bản tin sổ lệnh.
    pub timestamp: MarketTimestamp,
}

/// Enum đóng gói các loại sự kiện dữ liệu thị trường chuẩn hóa được luân chuyển trong hệ thống.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketEvent {
    /// Sự kiện khớp lệnh thực tế.
    Trade(Trade),
    /// Sự kiện cập nhật giá chào mua/chào bán tốt nhất.
    Quote(Quote),
}

impl MarketEvent {
    /// Trích xuất mốc thời gian phát sinh của sự kiện thị trường.
    ///
    /// # Giá trị trả về:
    /// - [`MarketTimestamp`]: Mốc thời gian UTC chuẩn hóa của Trade hoặc Quote.
    pub fn timestamp(&self) -> MarketTimestamp {
        match self {
            MarketEvent::Trade(trade) => trade.timestamp,
            MarketEvent::Quote(quote) => quote.timestamp,
        }
    }
}

impl Trade {
    /// Khởi tạo và thẩm định tính hợp lệ của một giao dịch khớp lệnh (`Trade`).
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán (tự động cắt khoảng trắng và chuẩn hóa viết hoa).
    /// - `price`: Mức giá khớp lệnh.
    /// - `volume`: Khối lượng khớp lệnh.
    /// - `timestamp`: Mốc thời gian chuẩn hóa của giao dịch.
    ///
    /// # Quy tắc thẩm định (Validation):
    /// - Mã chứng khoán không được rỗng sau khi cắt khoảng trắng.
    /// - Mức giá phải là số thực hữu hạn (`is_finite`) và lớn hơn 0.
    /// - Khối lượng phải là số thực hữu hạn và lớn hơn 0.
    ///
    /// # Lỗi trả về:
    /// - [`MarketDataError::InvalidSymbol`]: Nếu chuỗi `symbol` rỗng.
    /// - [`MarketDataError::InvalidPrice`]: Nếu `price <= 0` hoặc là NaN / Infinity.
    /// - [`MarketDataError::InvalidVolume`]: Nếu `volume <= 0` hoặc là NaN / Infinity.
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        timestamp: MarketTimestamp,
    ) -> Result<Self, MarketDataError> {
        if symbol.trim().is_empty() {
            return Err(MarketDataError::InvalidSymbol(
                "Mã chứng khoán không được rỗng".to_string(),
            ));
        }
        if !price.is_finite() || price <= 0.0 {
            return Err(MarketDataError::InvalidPrice(
                "giá không hợp lệ".to_string(),
            ));
        }
        if !volume.is_finite() || volume <= 0.0 {
            return Err(MarketDataError::InvalidVolume(
                "khối lượng không hợp lệ".to_string(),
            ));
        }
        Ok(Self {
            symbol: symbol.trim().to_uppercase(),
            price,
            volume,
            timestamp,
        })
    }
}

impl Quote {
    /// Khởi tạo và thẩm định tính toàn vẹn của bản tin giá chào mua/chào bán (`Quote`).
    ///
    /// # Tham số:
    /// - `symbol`: Mã chứng khoán.
    /// - `bid_price`: Mức giá đặt mua tốt nhất (Best Bid).
    /// - `bid_vol`: Khối lượng đặt mua tương ứng.
    /// - `ask_price`: Mức giá đặt bán tốt nhất (Best Ask).
    /// - `ask_vol`: Khối lượng đặt bán tương ứng.
    /// - `timestamp`: Mốc thời gian chuẩn hóa.
    ///
    /// # Quy tắc thẩm định (Validation):
    /// - Mã chứng khoán không được rỗng.
    /// - Giá và khối lượng phải là số hữu hạn không âm (`>= 0.0`).
    /// - Không cho phép cả `bid_price` và `ask_price` đồng thời bằng 0.
    /// - Chặn hiện tượng chéo giá (Crossed Market): Nếu cả hai mức giá đều lớn hơn 0 thì bắt buộc `bid_price < ask_price`.
    ///
    /// # Lỗi trả về:
    /// - [`MarketDataError::InvalidSymbol`]: Nếu `symbol` rỗng.
    /// - [`MarketDataError::InvalidPrice`]: Nếu giá âm, không hữu hạn, hoặc cả hai mức giá đều bằng 0.
    /// - [`MarketDataError::InvalidVolume`]: Nếu khối lượng âm hoặc không hữu hạn.
    /// - [`MarketDataError::CrossedMarket`]: Nếu xảy ra chéo giá (`bid_price >= ask_price > 0.0`).
    pub fn new(
        symbol: String,
        bid_price: f64,
        bid_vol: f64,
        ask_price: f64,
        ask_vol: f64,
        timestamp: MarketTimestamp,
    ) -> Result<Self, MarketDataError> {
        if symbol.trim().is_empty() {
            return Err(MarketDataError::InvalidSymbol(
                "Mã chứng khoán không được rỗng".to_string(),
            ));
        }
        if !bid_price.is_finite() || bid_price < 0.0 {
            return Err(MarketDataError::InvalidPrice(
                "bid_price không hợp lệ".to_string(),
            ));
        }
        if !ask_price.is_finite() || ask_price < 0.0 {
            return Err(MarketDataError::InvalidPrice(
                "ask_price không hợp lệ".to_string(),
            ));
        }
        if !bid_vol.is_finite() || bid_vol < 0.0 {
            return Err(MarketDataError::InvalidVolume(
                "bid_vol không hợp lệ".to_string(),
            ));
        }
        if !ask_vol.is_finite() || ask_vol < 0.0 {
            return Err(MarketDataError::InvalidVolume(
                "ask_vol không hợp lệ".to_string(),
            ));
        }
        if bid_price == 0.0 && ask_price == 0.0 {
            return Err(MarketDataError::InvalidPrice(
                "Cả bid_price và ask_price không thể đồng thời bằng 0".to_string(),
            ));
        }
        if bid_price > 0.0 && ask_price > 0.0 && bid_price >= ask_price {
            return Err(MarketDataError::CrossedMarket {
                symbol: symbol.trim().to_uppercase(),
                bid: bid_price,
                ask: ask_price,
            });
        }

        Ok(Self {
            symbol: symbol.trim().to_uppercase(),
            bid_price,
            bid_vol,
            ask_price,
            ask_vol,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_ts() -> MarketTimestamp {
        MarketTimestamp::from_epoch_secs(1725300000).expect("Hợp lệ")
    }

    #[test]
    fn test_trade_valid_and_normalization() {
        let trade = Trade::new("  vnm  ".to_string(), 65000.0, 100.0, dummy_ts()).expect("Hợp lệ");
        assert_eq!(trade.symbol, "VNM");
        assert_eq!(trade.price, 65000.0);
        assert_eq!(trade.volume, 100.0);
        assert_eq!(trade.timestamp, dummy_ts());
    }

    #[test]
    fn test_trade_invalid_symbol() {
        let err_empty = Trade::new("".to_string(), 65000.0, 100.0, dummy_ts());
        assert!(matches!(err_empty, Err(MarketDataError::InvalidSymbol(_))));

        let err_spaces = Trade::new("   ".to_string(), 65000.0, 100.0, dummy_ts());
        assert!(matches!(err_spaces, Err(MarketDataError::InvalidSymbol(_))));
    }

    #[test]
    fn test_trade_invalid_price() {
        // Giá bằng 0
        let err_zero = Trade::new("VNM".to_string(), 0.0, 100.0, dummy_ts());
        assert!(matches!(err_zero, Err(MarketDataError::InvalidPrice(_))));

        // Giá âm
        let err_neg = Trade::new("VNM".to_string(), -10.0, 100.0, dummy_ts());
        assert!(matches!(err_neg, Err(MarketDataError::InvalidPrice(_))));

        // Giá NaN hoặc Infinity
        let err_nan = Trade::new("VNM".to_string(), f64::NAN, 100.0, dummy_ts());
        assert!(matches!(err_nan, Err(MarketDataError::InvalidPrice(_))));

        let err_inf = Trade::new("VNM".to_string(), f64::INFINITY, 100.0, dummy_ts());
        assert!(matches!(err_inf, Err(MarketDataError::InvalidPrice(_))));
    }

    #[test]
    fn test_trade_invalid_volume() {
        let err_zero = Trade::new("VNM".to_string(), 65000.0, 0.0, dummy_ts());
        assert!(matches!(err_zero, Err(MarketDataError::InvalidVolume(_))));

        let err_neg = Trade::new("VNM".to_string(), 65000.0, -1.0, dummy_ts());
        assert!(matches!(err_neg, Err(MarketDataError::InvalidVolume(_))));

        let err_nan = Trade::new("VNM".to_string(), 65000.0, f64::NAN, dummy_ts());
        assert!(matches!(err_nan, Err(MarketDataError::InvalidVolume(_))));
    }

    #[test]
    fn test_trade_invalid_timestamp() {
        assert!(MarketTimestamp::from_epoch_millis(0).is_err());
        assert!(MarketTimestamp::from_epoch_millis(-100).is_err());
    }

    #[test]
    fn test_quote_valid_and_normalization() {
        let quote = Quote::new(
            " hpg ".to_string(),
            28500.0,
            200.0,
            28550.0,
            150.0,
            dummy_ts(),
        )
        .expect("Quote hợp lệ");
        assert_eq!(quote.symbol, "HPG");
        assert_eq!(quote.bid_price, 28500.0);
        assert_eq!(quote.bid_vol, 200.0);
        assert_eq!(quote.ask_price, 28550.0);
        assert_eq!(quote.ask_vol, 150.0);

        // Trường hợp kịch trần (ask_vol = 0.0 hoặc ask_price = 0.0)
        let quote_ceiling = Quote::new("HPG".to_string(), 30000.0, 500.0, 0.0, 0.0, dummy_ts())
            .expect("Quote tăng trần hợp lệ");
        assert_eq!(quote_ceiling.ask_vol, 0.0);
    }

    #[test]
    fn test_quote_invalid_fields() {
        // Mã rỗng
        let err_sym = Quote::new("".to_string(), 28500.0, 100.0, 28550.0, 100.0, dummy_ts());
        assert!(matches!(err_sym, Err(MarketDataError::InvalidSymbol(_))));

        // Giá âm hoặc NaN
        let err_bid_neg = Quote::new("HPG".to_string(), -1.0, 100.0, 28550.0, 100.0, dummy_ts());
        assert!(matches!(err_bid_neg, Err(MarketDataError::InvalidPrice(_))));

        let err_ask_nan = Quote::new(
            "HPG".to_string(),
            28500.0,
            100.0,
            f64::NAN,
            100.0,
            dummy_ts(),
        );
        assert!(matches!(err_ask_nan, Err(MarketDataError::InvalidPrice(_))));

        // Khối lượng âm hoặc NaN
        let err_bid_vol_neg = Quote::new(
            "HPG".to_string(),
            28500.0,
            -10.0,
            28550.0,
            100.0,
            dummy_ts(),
        );
        assert!(matches!(
            err_bid_vol_neg,
            Err(MarketDataError::InvalidVolume(_))
        ));

        // Timestamp âm
        assert!(MarketTimestamp::from_epoch_millis(-1).is_err());
    }

    #[test]
    fn test_market_event_variants() {
        let trade = Trade::new("HPG".to_string(), 28500.0, 100.0, dummy_ts()).unwrap();
        let event_trade = MarketEvent::Trade(trade.clone());

        let quote = Quote::new(
            "HPG".to_string(),
            28500.0,
            100.0,
            28550.0,
            100.0,
            dummy_ts(),
        )
        .unwrap();
        let event_quote = MarketEvent::Quote(quote.clone());

        match event_trade {
            MarketEvent::Trade(t) => assert_eq!(t, trade),
            _ => panic!("Expected Trade variant"),
        }

        match event_quote {
            MarketEvent::Quote(q) => assert_eq!(q, quote),
            _ => panic!("Expected Quote variant"),
        }
    }

    #[test]
    fn test_quote_crossed_market() {
        // Trường hợp 1: bid_price > ask_price
        let err_crossed = Quote::new(
            "HPG".to_string(),
            29000.0,
            100.0,
            28500.0,
            100.0,
            dummy_ts(),
        );
        match err_crossed {
            Err(MarketDataError::CrossedMarket { symbol, bid, ask }) => {
                assert_eq!(symbol, "HPG");
                assert_eq!(bid, 29000.0);
                assert_eq!(ask, 28500.0);
            }
            other => panic!("Expected CrossedMarket error, got {:?}", other),
        }

        // Trường hợp 2: bid_price == ask_price khi cả hai > 0
        let err_equal = Quote::new(
            "HPG".to_string(),
            28500.0,
            100.0,
            28550.0,
            100.0,
            dummy_ts(),
        );
        assert!(err_equal.is_ok());

        let err_strictly_equal = Quote::new(
            "HPG".to_string(),
            28500.0,
            100.0,
            28500.0,
            100.0,
            dummy_ts(),
        );
        assert!(matches!(
            err_strictly_equal,
            Err(MarketDataError::CrossedMarket { .. })
        ));
    }

    #[test]
    fn test_quote_both_bid_and_ask_zero() {
        let err_both_zero = Quote::new("HPG".to_string(), 0.0, 0.0, 0.0, 0.0, dummy_ts());
        assert!(matches!(
            err_both_zero,
            Err(MarketDataError::InvalidPrice(_))
        ));
    }

    #[test]
    fn test_quote_ceiling_and_floor_validity() {
        // Kịch trần: trắng bên bán (ask_price = 0.0, ask_vol = 0.0)
        let quote_ceiling = Quote::new("HPG".to_string(), 30000.0, 500.0, 0.0, 0.0, dummy_ts());
        assert!(quote_ceiling.is_ok());
        let q_ceil = quote_ceiling.unwrap();
        assert_eq!(q_ceil.bid_price, 30000.0);
        assert_eq!(q_ceil.ask_price, 0.0);

        // Kịch sàn: trắng bên mua (bid_price = 0.0, bid_vol = 0.0)
        let quote_floor = Quote::new("HPG".to_string(), 0.0, 0.0, 26000.0, 500.0, dummy_ts());
        assert!(quote_floor.is_ok());
        let q_flr = quote_floor.unwrap();
        assert_eq!(q_flr.bid_price, 0.0);
        assert_eq!(q_flr.ask_price, 26000.0);
    }
}
