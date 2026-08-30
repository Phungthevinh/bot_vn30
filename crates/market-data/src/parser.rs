use crate::RawMarketMessage;
use serde::{Deserialize, Serialize};
use vn30_domain::errors::MarketDataError;

// 1. Các struct sự kiện chi tiết
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeEvent {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteEvent {
    pub symbol: String,
    pub bid_price: f64,
    pub bid_vol: f64,
    pub ask_price: f64,
    pub ask_vol: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatEvent {
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeErrorEvent {
    pub code: String,
    pub message: String,
}

// 2. Enum đại diện cho tất cả các loại bản tin đã parse
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketMessage {
    Trade(TradeEvent),
    Quote(QuoteEvent),
    Heartbeat(HeartbeatEvent),
    #[serde(alias = "exchange_error", rename = "error")]
    ExchangeError(ExchangeErrorEvent),
    #[serde(skip)]
    Pong(Vec<u8>),
}

// 3. Struct Parser với các hàm parse chính
pub struct MarketDataParser;

impl MarketDataParser {
    /// Parse trực tiếp từ RawMarketMessage
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

    /// Parse từ chuỗi JSON text
    pub fn parse_json(text: &str) -> Result<MarketMessage, MarketDataError> {
        let msg = serde_json::from_str::<MarketMessage>(text)
            .map_err(|e| MarketDataError::ParseError(e.to_string()))?;

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let msg =
            MarketDataParser::parse_json(json_error).expect("Parse error tag should succeed");
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
        let raw_text = RawMarketMessage::Text(
            r#"{"type":"heartbeat","timestamp":1724900030}"#.to_string(),
        );
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
}
