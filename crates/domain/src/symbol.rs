use crate::errors::MarketDataError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instrument {
    Stock(StockSymbol),
    IndexFuture(FutureContract),
    Index(IndexSymbol),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockSymbol {
    pub symbol: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureContract {
    pub underlying: String,
    pub year: u8,
    pub month: u8,
    pub canonical_symbol: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSymbol {
    pub symbol: String,
}

impl StockSymbol {
    pub fn new(symbol: &str) -> Result<Self, MarketDataError> {
        const EXCHANGES: &[&str] = &["HOSE", "HNX", "UPCOM", "HSX"];
        if symbol.is_empty() {
            return Err(MarketDataError::InvalidData {
                symbol: symbol.to_string(),
                reason: "Symbol is empty".to_string(),
            });
        }

        for part in symbol.split(|c| c == ':' || c == '.' || c == '/') {
            let trimmed = part.trim().to_uppercase();

            if EXCHANGES.contains(&trimmed.as_str()) {
                continue;
            }

            if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(MarketDataError::InvalidData {
                    symbol: symbol.to_string(),
                    reason: "Stock symbol must be 3 alphabetic characters".to_string(),
                });
            }
            return Ok(Self { symbol: trimmed });
        }

        Err(MarketDataError::InvalidData {
            symbol: symbol.to_string(),
            reason: "Symbol is not in any exchange".to_string(),
        })
    }
}

impl FutureContract {
    pub fn new(raw: &str) -> Result<Self, MarketDataError> {
        let raw = raw.trim().to_uppercase();

        if raw.len() != 9 || !raw.starts_with("VN30F") {
            return Err(MarketDataError::InvalidData {
                symbol: raw.to_string(),
                reason: "Symbol is not valid".to_string(),
            });
        }

        let year = raw[5..7]
            .parse::<u8>()
            .map_err(|e| MarketDataError::InvalidData {
                symbol: raw.to_string(),
                reason: format!("Year is invalid: {}", e),
            })?;
        let month = raw[7..9]
            .parse::<u8>()
            .map_err(|e| MarketDataError::InvalidData {
                symbol: raw.to_string(),
                reason: format!("Month is invalid: {}", e),
            })?;

        if !(1..=12).contains(&month) {
            return Err(MarketDataError::InvalidData {
                symbol: raw.to_string(),
                reason: "Month must be between 1 and 12".to_string(),
            });
        }
        Ok(Self {
            underlying: "VN30F".to_string(),
            year,
            month,
            canonical_symbol: raw.to_string(),
        })
    }
}

impl IndexSymbol {
    pub fn new(symbol: &str) -> Result<Self, MarketDataError> {
        let res = match symbol.trim().to_uppercase().as_str() {
            "VN30" => "VN30",
            "VNINDEX" => "VNINDEX",
            _ => {
                return Err(MarketDataError::InvalidData {
                    symbol: symbol.to_string(),
                    reason: "Index is not valid".to_string(),
                })
            }
        };

        Ok(Self {
            symbol: res.to_string(),
        })
    }
}

impl Instrument {
    pub fn parse_canonical(s: &str) -> Result<Self, MarketDataError> {
        let s = s.trim().to_uppercase();
        if s == "VN30" || s == "VNINDEX" {
            return Ok(Self::Index(IndexSymbol::new(&s)?));
        } else if s.starts_with("VN30F") {
            return Ok(Self::IndexFuture(FutureContract::new(&s)?));
        } else {
            return Ok(Self::Stock(StockSymbol::new(&s)?));
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Instrument::Stock(symbol) => &symbol.symbol,
            Instrument::IndexFuture(contract) => &contract.canonical_symbol,
            Instrument::Index(index) => &index.symbol,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_symbol_valid_and_exchange_strip() {
        assert_eq!(StockSymbol::new("HPG").unwrap().symbol, "HPG");
        assert_eq!(StockSymbol::new("  vnm  ").unwrap().symbol, "VNM");
        assert_eq!(StockSymbol::new("fpt").unwrap().symbol, "FPT");

        // Tiền tố / hậu tố sàn
        assert_eq!(StockSymbol::new("HOSE:HPG").unwrap().symbol, "HPG");
        assert_eq!(StockSymbol::new("HPG.HOSE").unwrap().symbol, "HPG");
        assert_eq!(StockSymbol::new("UPCOM:BSR").unwrap().symbol, "BSR");
        assert_eq!(StockSymbol::new("HNX/SHB").unwrap().symbol, "SHB");
    }

    #[test]
    fn test_stock_symbol_invalid() {
        assert!(StockSymbol::new("").is_err());
        assert!(StockSymbol::new("   ").is_err());
        assert!(StockSymbol::new("HP").is_err());
        assert!(StockSymbol::new("HPGG").is_err());
        assert!(StockSymbol::new("123").is_err());
        assert!(StockSymbol::new("HP@").is_err());
        assert!(StockSymbol::new("HOSE").is_err());
    }

    #[test]
    fn test_future_contract_valid() {
        let fc = FutureContract::new("VN30F2409").unwrap();
        assert_eq!(fc.underlying, "VN30F");
        assert_eq!(fc.year, 24);
        assert_eq!(fc.month, 9);
        assert_eq!(fc.canonical_symbol, "VN30F2409");

        let fc_lower = FutureContract::new("vn30f2512").unwrap();
        assert_eq!(fc_lower.canonical_symbol, "VN30F2512");
        assert_eq!(fc_lower.month, 12);
    }

    #[test]
    fn test_future_contract_invalid() {
        assert!(FutureContract::new("VN30F").is_err());
        assert!(FutureContract::new("VN30F2").is_err());
        assert!(FutureContract::new("VN30F240").is_err());
        assert!(FutureContract::new("VN30F2413").is_err());
        assert!(FutureContract::new("VN30F2400").is_err());
        assert!(FutureContract::new("VN100F2409").is_err());
    }

    #[test]
    fn test_index_symbol() {
        assert_eq!(IndexSymbol::new("VN30").unwrap().symbol, "VN30");
        assert_eq!(IndexSymbol::new("  vnindex  ").unwrap().symbol, "VNINDEX");
        assert!(IndexSymbol::new("VN50").is_err());
    }

    #[test]
    fn test_instrument_parse_canonical() {
        let stock = Instrument::parse_canonical("HPG").unwrap();
        assert_eq!(stock.as_str(), "HPG");
        assert!(matches!(stock, Instrument::Stock(_)));

        let future = Instrument::parse_canonical("VN30F2409").unwrap();
        assert_eq!(future.as_str(), "VN30F2409");
        assert!(matches!(future, Instrument::IndexFuture(_)));

        let index = Instrument::parse_canonical("VN30").unwrap();
        assert_eq!(index.as_str(), "VN30");
        assert!(matches!(index, Instrument::Index(_)));

        // Kiểm tra xử lý chuỗi rác khi parse qua Instrument
        assert!(Instrument::parse_canonical("INVALID_LONG_TICKER").is_err());
        assert!(Instrument::parse_canonical("VN30F").is_err());
    }
}
