use serde::{Deserialize, Serialize};

use crate::errors::MarketDataError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid_price: f64,
    pub bid_vol: f64,
    pub ask_price: f64,
    pub ask_vol: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketEvent {
    Trade(Trade),
    Quote(Quote),
}

impl Trade {
    pub fn new(
        symbol: String,
        price: f64,
        volume: f64,
        timestamp: i64,
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

        if timestamp <= 0 {
            return Err(MarketDataError::InvalidTimestamp(
                "timestamp không hợp lệ".to_string(),
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
    pub fn new(
        symbol: String,
        bid_price: f64,
        bid_vol: f64,
        ask_price: f64,
        ask_vol: f64,
        timestamp: i64,
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
        if timestamp <= 0 {
            return Err(MarketDataError::InvalidTimestamp(
                "timestamp không hợp lệ".to_string(),
            ));
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

    #[test]
    fn test_trade_valid_and_normalization() {
        let trade = Trade::new("  vnm  ".to_string(), 65000.0, 100.0, 1725300000)
            .expect("Hợp lệ");
        assert_eq!(trade.symbol, "VNM");
        assert_eq!(trade.price, 65000.0);
        assert_eq!(trade.volume, 100.0);
        assert_eq!(trade.timestamp, 1725300000);
    }

    #[test]
    fn test_trade_invalid_symbol() {
        let err_empty = Trade::new("".to_string(), 65000.0, 100.0, 1725300000);
        assert!(matches!(err_empty, Err(MarketDataError::InvalidSymbol(_))));

        let err_spaces = Trade::new("   ".to_string(), 65000.0, 100.0, 1725300000);
        assert!(matches!(err_spaces, Err(MarketDataError::InvalidSymbol(_))));
    }

    #[test]
    fn test_trade_invalid_price() {
        // Giá bằng 0
        let err_zero = Trade::new("VNM".to_string(), 0.0, 100.0, 1725300000);
        assert!(matches!(err_zero, Err(MarketDataError::InvalidPrice(_))));

        // Giá âm
        let err_neg = Trade::new("VNM".to_string(), -10.0, 100.0, 1725300000);
        assert!(matches!(err_neg, Err(MarketDataError::InvalidPrice(_))));

        // Giá NaN hoặc Infinity
        let err_nan = Trade::new("VNM".to_string(), f64::NAN, 100.0, 1725300000);
        assert!(matches!(err_nan, Err(MarketDataError::InvalidPrice(_))));

        let err_inf = Trade::new("VNM".to_string(), f64::INFINITY, 100.0, 1725300000);
        assert!(matches!(err_inf, Err(MarketDataError::InvalidPrice(_))));
    }

    #[test]
    fn test_trade_invalid_volume() {
        let err_zero = Trade::new("VNM".to_string(), 65000.0, 0.0, 1725300000);
        assert!(matches!(err_zero, Err(MarketDataError::InvalidVolume(_))));

        let err_neg = Trade::new("VNM".to_string(), 65000.0, -1.0, 1725300000);
        assert!(matches!(err_neg, Err(MarketDataError::InvalidVolume(_))));

        let err_nan = Trade::new("VNM".to_string(), 65000.0, f64::NAN, 1725300000);
        assert!(matches!(err_nan, Err(MarketDataError::InvalidVolume(_))));
    }

    #[test]
    fn test_trade_invalid_timestamp() {
        let err_zero = Trade::new("VNM".to_string(), 65000.0, 100.0, 0);
        assert!(matches!(err_zero, Err(MarketDataError::InvalidTimestamp(_))));

        let err_neg = Trade::new("VNM".to_string(), 65000.0, 100.0, -100);
        assert!(matches!(err_neg, Err(MarketDataError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_quote_valid_and_normalization() {
        let quote = Quote::new(" hpg ".to_string(), 28500.0, 200.0, 28550.0, 150.0, 1725300000)
            .expect("Quote hợp lệ");
        assert_eq!(quote.symbol, "HPG");
        assert_eq!(quote.bid_price, 28500.0);
        assert_eq!(quote.bid_vol, 200.0);
        assert_eq!(quote.ask_price, 28550.0);
        assert_eq!(quote.ask_vol, 150.0);

        // Trường hợp kịch trần (ask_vol = 0.0 hoặc ask_price = 0.0)
        let quote_ceiling = Quote::new("HPG".to_string(), 30000.0, 500.0, 0.0, 0.0, 1725300000)
            .expect("Quote tăng trần hợp lệ");
        assert_eq!(quote_ceiling.ask_vol, 0.0);
    }

    #[test]
    fn test_quote_invalid_fields() {
        // Mã rỗng
        let err_sym = Quote::new("".to_string(), 28500.0, 100.0, 28550.0, 100.0, 1725300000);
        assert!(matches!(err_sym, Err(MarketDataError::InvalidSymbol(_))));

        // Giá âm hoặc NaN
        let err_bid_neg = Quote::new("HPG".to_string(), -1.0, 100.0, 28550.0, 100.0, 1725300000);
        assert!(matches!(err_bid_neg, Err(MarketDataError::InvalidPrice(_))));

        let err_ask_nan = Quote::new("HPG".to_string(), 28500.0, 100.0, f64::NAN, 100.0, 1725300000);
        assert!(matches!(err_ask_nan, Err(MarketDataError::InvalidPrice(_))));

        // Khối lượng âm hoặc NaN
        let err_bid_vol_neg = Quote::new("HPG".to_string(), 28500.0, -10.0, 28550.0, 100.0, 1725300000);
        assert!(matches!(err_bid_vol_neg, Err(MarketDataError::InvalidVolume(_))));

        // Timestamp âm
        let err_ts = Quote::new("HPG".to_string(), 28500.0, 100.0, 28550.0, 100.0, -1);
        assert!(matches!(err_ts, Err(MarketDataError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_market_event_variants() {
        let trade = Trade::new("HPG".to_string(), 28500.0, 100.0, 1725300000).unwrap();
        let event_trade = MarketEvent::Trade(trade.clone());

        let quote = Quote::new("HPG".to_string(), 28500.0, 100.0, 28550.0, 100.0, 1725300000).unwrap();
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
}
