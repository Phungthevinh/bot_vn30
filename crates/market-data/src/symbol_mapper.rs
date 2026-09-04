use std::collections::HashMap;
use vn30_domain::errors::MarketDataError;
use vn30_domain::symbol::Instrument;

#[derive(Debug, Clone, Default)]
pub struct SymbolMapper {
    /// Bảng tra cứu alias (ví dụ: "VN30F1M" -> "VN30F2409")
    aliases: HashMap<String, String>,
}

impl SymbolMapper {
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    pub fn register_alias(&mut self, alias: &str, canonical: &str) {
        let alias_upper = alias.trim().to_uppercase();
        let canonical_upper = canonical.trim().to_uppercase();
        self.aliases.insert(alias_upper, canonical_upper);
    }

    pub fn map(&self, raw: &str) -> Result<Instrument, MarketDataError> {
        let raw = raw.trim().to_uppercase();

        if let Some(target_symbol) = self.aliases.get(&raw) {
            Instrument::parse_canonical(target_symbol)
        } else {
            Instrument::parse_canonical(&raw)
        }
    }

    pub fn map_to_canonical(&self, raw: &str) -> Result<String, MarketDataError> {
        self.map(raw).map(|inst| inst.as_str().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_stock_without_alias() {
        let mapper = SymbolMapper::new();

        let inst = mapper.map("HPG").unwrap();
        assert_eq!(inst.as_str(), "HPG");
        assert!(matches!(inst, Instrument::Stock(_)));

        // Xử lý tiền tố và hậu tố sàn
        let inst_hose = mapper.map("HOSE:HPG").unwrap();
        assert_eq!(inst_hose.as_str(), "HPG");

        let inst_lower = mapper.map("  vnm.hose  ").unwrap();
        assert_eq!(inst_lower.as_str(), "VNM");

        // Kiểm tra helper map_to_canonical
        assert_eq!(mapper.map_to_canonical("UPCOM:BSR").unwrap(), "BSR");
    }

    #[test]
    fn test_map_index() {
        let mapper = SymbolMapper::new();

        let inst = mapper.map("VN30").unwrap();
        assert_eq!(inst.as_str(), "VN30");
        assert!(matches!(inst, Instrument::Index(_)));

        let inst_vnindex = mapper.map("  vnindex  ").unwrap();
        assert_eq!(inst_vnindex.as_str(), "VNINDEX");
        assert_eq!(mapper.map_to_canonical("VN30").unwrap(), "VN30");
    }

    #[test]
    fn test_map_future_canonical() {
        let mapper = SymbolMapper::new();

        let inst = mapper.map("VN30F2409").unwrap();
        assert_eq!(inst.as_str(), "VN30F2409");
        assert!(matches!(inst, Instrument::IndexFuture(_)));

        let inst_lower = mapper.map("vn30f2512").unwrap();
        assert_eq!(inst_lower.as_str(), "VN30F2512");
        assert_eq!(
            mapper.map_to_canonical("VN30F2409").unwrap(),
            "VN30F2409"
        );
    }

    #[test]
    fn test_map_future_with_alias() {
        let mut mapper = SymbolMapper::new();
        mapper.register_alias("VN30F1M", "VN30F2409");
        mapper.register_alias("VN30F2M", "VN30F2410");

        // Map qua alias
        let inst = mapper.map("VN30F1M").unwrap();
        assert_eq!(inst.as_str(), "VN30F2409");
        assert!(matches!(inst, Instrument::IndexFuture(_)));

        // Không phân biệt hoa thường và khoảng trắng
        let inst_case = mapper.map("  vn30f1m  ").unwrap();
        assert_eq!(inst_case.as_str(), "VN30F2409");

        // Map alias thứ 2 qua helper string
        assert_eq!(
            mapper.map_to_canonical("vn30f2m").unwrap(),
            "VN30F2410"
        );
    }

    #[test]
    fn test_unregistered_alias_fails() {
        let mapper = SymbolMapper::new();
        // Chưa đăng ký alias VN30F1M -> Phải báo lỗi Fail-fast
        assert!(mapper.map("VN30F1M").is_err());
        assert!(mapper.map_to_canonical("VN30F1M").is_err());
    }

    #[test]
    fn test_map_invalid_symbols() {
        let mapper = SymbolMapper::new();
        assert!(mapper.map("").is_err());
        assert!(mapper.map("   ").is_err());
        assert!(mapper.map("12345").is_err());
        assert!(mapper.map("HP@").is_err());
        assert!(mapper.map("VN30F").is_err()); // Thiếu expiry
        assert!(mapper.map("VN30F2413").is_err()); // Tháng 13
        assert!(mapper.map("VN30F2400").is_err()); // Tháng 0
    }
}
