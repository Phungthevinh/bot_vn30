use crate::RawMarketMessage;
use serde::{Deserialize, Serialize};
use vn30_domain::errors::MarketDataError;
use vn30_domain::market::{MarketEvent, Quote, Trade};
use vn30_domain::timestamp::MarketTimestamp;

/// Sự kiện khớp lệnh thô (Trade Event) được giải tuần tự hóa từ gói tin JSON của sàn giao dịch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeEvent {
    /// Mã chứng khoán nhận được từ sàn (ví dụ: `"HPG"`, `"VN30F2409"`).
    pub symbol: String,
    /// Mức giá khớp lệnh thực tế.
    pub price: f64,
    /// Khối lượng khớp lệnh thực tế.
    pub volume: f64,
    /// Mốc thời gian phát sinh giao dịch dạng số nguyên epoch thô (giây hoặc mili-giây).
    pub timestamp: i64,
}

/// Sự kiện cập nhật giá chào mua và chào bán tốt nhất (BBO Quote Event) từ sàn giao dịch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteEvent {
    /// Mã chứng khoán nhận được từ sàn.
    pub symbol: String,
    /// Mức giá đặt mua cao nhất (Best Bid Price).
    pub bid_price: f64,
    /// Khối lượng đặt mua tương ứng.
    pub bid_vol: f64,
    /// Mức giá đặt bán thấp nhất (Best Ask Price).
    pub ask_price: f64,
    /// Khối lượng đặt bán tương ứng.
    pub ask_vol: f64,
    /// Mốc thời gian cập nhật sổ lệnh dạng số nguyên epoch thô.
    pub timestamp: i64,
}

/// Sự kiện nhịp tim định kỳ (Heartbeat Event) do máy chủ sàn gửi về để giữ kết nối sống.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatEvent {
    /// Mốc thời gian của tín hiệu Heartbeat dạng số nguyên epoch thô.
    pub timestamp: i64,
}

/// Sự kiện thông báo lỗi phát sinh từ phía sàn giao dịch (Exchange Error Event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeErrorEvent {
    /// Mã định danh lỗi từ sàn (ví dụ: `"AUTH_FAILED"`, `"RATE_LIMIT"`).
    pub code: String,
    /// Thông điệp chi tiết mô tả nguyên nhân lỗi do sàn trả về.
    pub message: String,
}

/// Enum đóng gói toàn bộ các loại bản tin thị trường sau khi phân tích cú pháp từ stream.
///
/// Phân loại dựa trên trường `"type"` trong payload JSON (`snake_case`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketMessage {
    /// Bản tin giao dịch khớp lệnh thực tế.
    Trade(TradeEvent),
    /// Bản tin cập nhật giá chào mua / chào bán tốt nhất.
    Quote(QuoteEvent),
    /// Bản tin nhịp tim giữ kết nối (Heartbeat / Keep-alive).
    Heartbeat(HeartbeatEvent),
    /// Bản tin cảnh báo lỗi từ sàn giao dịch (hỗ trợ cả tag `"error"` và alias `"exchange_error"`).
    #[serde(alias = "exchange_error", rename = "error")]
    ExchangeError(ExchangeErrorEvent),
    /// Bản tin Pong phản hồi cho các frame Ping ở tầng giao thức WebSocket.
    #[serde(skip)]
    Pong(Vec<u8>),
}

/// Bộ phân tích cú pháp dữ liệu thị trường (Parser) chịu trách nhiệm giải mã các frame thô từ sàn.
pub struct MarketDataParser;

impl MarketDataParser {
    /// Phân tích trực tiếp từ bản tin thô [`RawMarketMessage`] thành [`MarketMessage`].
    ///
    /// # Tham số:
    /// - `raw`: Tham chiếu tới bản tin WebSocket thô vừa nhận được.
    ///
    /// # Giá trị trả về:
    /// - `Ok(MarketMessage)`: Bản tin thị trường đã được bóc tách định dạng.
    /// - `Err(MarketDataError::ParseError)`: Nếu frame ở định dạng nhị phân không hỗ trợ hoặc lỗi cú pháp JSON.
    pub fn parse(raw: &RawMarketMessage) -> Result<MarketMessage, MarketDataError> {
        match raw {
            RawMarketMessage::Text(text) => Self::parse_json(text),
            RawMarketMessage::Binary(_) => Err(MarketDataError::ParseError(
                "Binary frame format not supported".to_string(),
            )),
            RawMarketMessage::Ping(data) => Ok(MarketMessage::Pong(data.clone())),
            RawMarketMessage::Pong(data) => Ok(MarketMessage::Pong(data.clone())),
        }
    }

    /// Phân tích cú pháp chuỗi JSON text thành đối tượng [`MarketMessage`].
    ///
    /// # Tham số:
    /// - `text`: Chuỗi JSON nhận được từ máy chủ sàn (chứa trường `"type"` nhận diện: trade, quote, heartbeat, error).
    ///
    /// # Giá trị trả về:
    /// - `Ok(MarketMessage)`: Đối tượng bản tin phân loại tương ứng.
    /// - `Err(MarketDataError::ParseError)`: Nếu chuỗi JSON sai cú pháp hoặc thiếu các trường dữ liệu bắt buộc.
    pub fn parse_json(text: &str) -> Result<MarketMessage, MarketDataError> {
        let msg = serde_json::from_str::<MarketMessage>(text)
            .map_err(|e| MarketDataError::ParseError(e.to_string()))?;

        Ok(msg)
    }
}

impl MarketMessage {
    /// Chuyển đổi (adapt) bản tin sàn [`MarketMessage`] sang sự kiện miền lõi [`MarketEvent`] (nếu có).
    ///
    /// # Cơ chế:
    /// - [`MarketMessage::Trade`]: Chuẩn hóa timestamp sang [`MarketTimestamp`], thẩm định qua [`Trade::new`], trả về `Ok(Some(MarketEvent::Trade))`.
    /// - [`MarketMessage::Quote`]: Chuẩn hóa timestamp, thẩm định qua [`Quote::new`] (kiểm tra chéo giá crossed market, giá không âm...), trả về `Ok(Some(MarketEvent::Quote))`.
    /// - [`MarketMessage::Heartbeat`] / [`MarketMessage::Pong`]: Trả về `Ok(None)` vì là bản tin giao thức, không đẩy vào pipeline phân tích.
    /// - [`MarketMessage::ExchangeError`]: Chuyển đổi thành lỗi [`MarketDataError::ParseError`] mang thông điệp từ sàn.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Some(MarketEvent))`: Sự kiện thị trường chuẩn hóa sẵn sàng chuyển vào State Store / Indicator Engine.
    /// - `Ok(None)`: Bản tin điều khiển kết nối không cần xử lý downstream.
    /// - `Err(MarketDataError)`: Nếu dữ liệu trong sự kiện vi phạm các quy tắc nghiệp vụ thị trường.
    pub fn try_into_market_event(&self) -> Result<Option<MarketEvent>, MarketDataError> {
        match self {
            MarketMessage::Trade(trade) => {
                let ts = MarketTimestamp::from_raw_epoch(trade.timestamp)?;
                let domain_trade = Trade::new(trade.symbol.clone(), trade.price, trade.volume, ts)?;
                Ok(Some(MarketEvent::Trade(domain_trade)))
            }
            MarketMessage::Quote(quote) => {
                let ts = MarketTimestamp::from_raw_epoch(quote.timestamp)?;
                let domain_quote = Quote::new(
                    quote.symbol.clone(),
                    quote.bid_price,
                    quote.bid_vol,
                    quote.ask_price,
                    quote.ask_vol,
                    ts,
                )?;
                Ok(Some(MarketEvent::Quote(domain_quote)))
            }
            MarketMessage::Heartbeat(_) => Ok(None),
            MarketMessage::ExchangeError(err) => {
                Err(MarketDataError::ParseError(err.message.clone()))
            }
            MarketMessage::Pong(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_try_into_market_event_quote_crossed_market_fails() {
        let msg = MarketMessage::Quote(QuoteEvent {
            symbol: "HPG".to_string(),
            bid_price: 29000.0,
            bid_vol: 100.0,
            ask_price: 28500.0,
            ask_vol: 100.0,
            timestamp: 1724900000,
        });

        let result = msg.try_into_market_event();
        assert!(matches!(
            result,
            Err(MarketDataError::CrossedMarket { symbol, bid, ask })
                if symbol == "HPG" && bid == 29000.0 && ask == 28500.0
        ));
    }

    #[test]
    fn test_try_into_market_event_quote_both_zero_fails() {
        let msg = MarketMessage::Quote(QuoteEvent {
            symbol: "HPG".to_string(),
            bid_price: 0.0,
            bid_vol: 0.0,
            ask_price: 0.0,
            ask_vol: 0.0,
            timestamp: 1724900000,
        });

        let result = msg.try_into_market_event();
        assert!(matches!(result, Err(MarketDataError::InvalidPrice(_))));
    }

    #[test]
    fn test_try_into_market_event_quote_ceiling_and_floor_success() {
        // Kịch trần: trắng bên bán (ask_price = 0.0, ask_vol = 0.0)
        let ceiling_msg = MarketMessage::Quote(QuoteEvent {
            symbol: "HPG".to_string(),
            bid_price: 30000.0,
            bid_vol: 500.0,
            ask_price: 0.0,
            ask_vol: 0.0,
            timestamp: 1724900000,
        });
        let ceiling_event = ceiling_msg.try_into_market_event();
        assert!(ceiling_event.is_ok());
        assert!(ceiling_event.unwrap().is_some());

        // Kịch sàn: trắng bên mua (bid_price = 0.0, bid_vol = 0.0)
        let floor_msg = MarketMessage::Quote(QuoteEvent {
            symbol: "HPG".to_string(),
            bid_price: 0.0,
            bid_vol: 0.0,
            ask_price: 26000.0,
            ask_vol: 500.0,
            timestamp: 1724900000,
        });
        let floor_event = floor_msg.try_into_market_event();
        assert!(floor_event.is_ok());
        assert!(floor_event.unwrap().is_some());
    }

    #[test]
    fn test_parse_trade_event_success() {
        let json = r#"{
            "type": "trade",
            "symbol": "VN30F2409",
            "price": 1250.5,
            "volume": 15.0,
            "timestamp": 1724900000
        }"#;

        let msg = MarketDataParser::parse_json(json).expect("Parse trade json should succeed");
        match msg {
            MarketMessage::Trade(trade) => {
                assert_eq!(trade.symbol, "VN30F2409");
                assert_eq!(trade.price, 1250.5);
                assert_eq!(trade.volume, 15.0);
                assert_eq!(trade.timestamp, 1724900000);
            }
            other => panic!("Expected MarketMessage::Trade, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_quote_event_success() {
        let json = r#"{
            "type": "quote",
            "symbol": "TCB",
            "bid_price": 24.5,
            "bid_vol": 1000.0,
            "ask_price": 24.6,
            "ask_vol": 500.0,
            "timestamp": 1724900010
        }"#;

        let msg = MarketDataParser::parse_json(json).expect("Parse quote json should succeed");
        match msg {
            MarketMessage::Quote(quote) => {
                assert_eq!(quote.symbol, "TCB");
                assert_eq!(quote.bid_price, 24.5);
                assert_eq!(quote.bid_vol, 1000.0);
                assert_eq!(quote.ask_price, 24.6);
                assert_eq!(quote.ask_vol, 500.0);
                assert_eq!(quote.timestamp, 1724900010);
            }
            other => panic!("Expected MarketMessage::Quote, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_heartbeat_event_success() {
        let json = r#"{
            "type": "heartbeat",
            "timestamp": 1724900020
        }"#;

        let msg = MarketDataParser::parse_json(json).expect("Parse heartbeat json should succeed");
        match msg {
            MarketMessage::Heartbeat(hb) => {
                assert_eq!(hb.timestamp, 1724900020);
            }
            other => panic!("Expected MarketMessage::Heartbeat, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_exchange_error_event_with_rename_and_alias() {
        let json_error = r#"{
            "type": "error",
            "code": "AUTH_FAILED",
            "message": "Token expired"
        }"#;

        let msg = MarketDataParser::parse_json(json_error).expect("Parse error tag should succeed");
        match msg {
            MarketMessage::ExchangeError(err) => {
                assert_eq!(err.code, "AUTH_FAILED");
                assert_eq!(err.message, "Token expired");
            }
            other => panic!("Expected MarketMessage::ExchangeError, got {:?}", other),
        }

        let json_exchange_error = r#"{
            "type": "exchange_error",
            "code": "INVALID_REQ",
            "message": "Bad request"
        }"#;

        let msg_alias = MarketDataParser::parse_json(json_exchange_error)
            .expect("Parse exchange_error tag alias should succeed");
        match msg_alias {
            MarketMessage::ExchangeError(err) => {
                assert_eq!(err.code, "INVALID_REQ");
                assert_eq!(err.message, "Bad request");
            }
            other => panic!("Expected MarketMessage::ExchangeError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_raw_market_message_variants() {
        // Text variant
        let raw_text =
            RawMarketMessage::Text(r#"{"type":"heartbeat","timestamp":1724900030}"#.to_string());
        let parsed_text = MarketDataParser::parse(&raw_text).expect("Parse text raw should work");
        assert_eq!(
            parsed_text,
            MarketMessage::Heartbeat(HeartbeatEvent {
                timestamp: 1724900030
            })
        );

        // Ping variant
        let raw_ping = RawMarketMessage::Ping(vec![1, 2, 3]);
        let parsed_ping = MarketDataParser::parse(&raw_ping).expect("Ping should map to Pong");
        assert_eq!(parsed_ping, MarketMessage::Pong(vec![1, 2, 3]));

        // Pong variant
        let raw_pong = RawMarketMessage::Pong(vec![4, 5, 6]);
        let parsed_pong = MarketDataParser::parse(&raw_pong).expect("Pong should map to Pong");
        assert_eq!(parsed_pong, MarketMessage::Pong(vec![4, 5, 6]));

        // Binary variant (currently unsupported)
        let raw_binary = RawMarketMessage::Binary(vec![0xAA, 0xBB]);
        let result_bin = MarketDataParser::parse(&raw_binary);
        assert!(result_bin.is_err());
        match result_bin.unwrap_err() {
            MarketDataError::ParseError(msg) => {
                assert!(msg.contains("Binary frame format not supported"));
            }
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_malformed_json_returns_parse_error() {
        let bad_json = "{ invalid_json_syntax ";
        let result = MarketDataParser::parse_json(bad_json);
        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ParseError(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_missing_field_returns_parse_error() {
        // Trade without volume
        let incomplete_trade = r#"{
            "type": "trade",
            "symbol": "HPG",
            "price": 28.5,
            "timestamp": 1724900040
        }"#;

        let result = MarketDataParser::parse_json(incomplete_trade);
        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ParseError(msg) => {
                assert!(msg.contains("missing field `volume`"));
            }
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unknown_type_returns_parse_error() {
        let unknown_type = r#"{
            "type": "news_alert",
            "title": "Market opens high"
        }"#;

        let result = MarketDataParser::parse_json(unknown_type);
        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ParseError(msg) => {
                assert!(msg.contains("unknown variant `news_alert`"));
            }
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn test_try_into_market_event_trade_success() {
        let msg = MarketMessage::Trade(TradeEvent {
            symbol: "  hpg  ".to_string(),
            price: 28500.0,
            volume: 100.0,
            timestamp: 1724900000,
        });

        let event = msg
            .try_into_market_event()
            .expect("Chuyển đổi Trade thành công");
        assert!(event.is_some());
        match event.unwrap() {
            MarketEvent::Trade(trade) => {
                assert_eq!(trade.symbol, "HPG");
                assert_eq!(trade.price, 28500.0);
                assert_eq!(trade.volume, 100.0);
                assert_eq!(trade.timestamp.timestamp_secs(), 1724900000);
            }
            _ => panic!("Expected MarketEvent::Trade"),
        }
    }

    #[test]
    fn test_try_into_market_event_quote_success() {
        let msg = MarketMessage::Quote(QuoteEvent {
            symbol: "vnm".to_string(),
            bid_price: 65000.0,
            bid_vol: 200.0,
            ask_price: 65100.0,
            ask_vol: 150.0,
            timestamp: 1724900000,
        });

        let event = msg
            .try_into_market_event()
            .expect("Chuyển đổi Quote thành công");
        assert!(event.is_some());
        match event.unwrap() {
            MarketEvent::Quote(quote) => {
                assert_eq!(quote.symbol, "VNM");
                assert_eq!(quote.bid_price, 65000.0);
                assert_eq!(quote.bid_vol, 200.0);
                assert_eq!(quote.ask_price, 65100.0);
                assert_eq!(quote.ask_vol, 150.0);
            }
            _ => panic!("Expected MarketEvent::Quote"),
        }
    }

    #[test]
    fn test_try_into_market_event_trade_invalid_price_fails() {
        let msg = MarketMessage::Trade(TradeEvent {
            symbol: "HPG".to_string(),
            price: -100.0,
            volume: 100.0,
            timestamp: 1724900000,
        });

        let result = msg.try_into_market_event();
        assert!(matches!(result, Err(MarketDataError::InvalidPrice(_))));
    }

    #[test]
    fn test_try_into_market_event_heartbeat_and_pong_none() {
        let hb_msg = MarketMessage::Heartbeat(HeartbeatEvent {
            timestamp: 1724900000,
        });
        assert_eq!(hb_msg.try_into_market_event().unwrap(), None);

        let pong_msg = MarketMessage::Pong(vec![1, 2, 3]);
        assert_eq!(pong_msg.try_into_market_event().unwrap(), None);
    }

    #[test]
    fn test_try_into_market_event_exchange_error_returns_err() {
        let err_msg = MarketMessage::ExchangeError(ExchangeErrorEvent {
            code: "ERR_TIMEOUT".to_string(),
            message: "Gateway timeout".to_string(),
        });

        let result = err_msg.try_into_market_event();
        assert!(result.is_err());
        match result.unwrap_err() {
            MarketDataError::ParseError(msg) => {
                assert_eq!(msg, "Gateway timeout");
            }
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }
}
