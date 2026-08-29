use serde_json::json;
use std::collections::HashSet;
use vn30_domain::errors::MarketDataError;

#[derive(Debug, Clone, Default)]
pub struct SubscriptionManager {
    active_symbols: HashSet<String>,
}

impl SubscriptionManager {
    /// Khởi tạo rỗng
    pub fn new() -> Self {
        Self {
            active_symbols: HashSet::new(),
        }
    }

    /// Khởi tạo với danh sách symbols ban đầu
    pub fn with_symbols<I, S>(symbols: I) -> Result<(Self, Option<String>), MarketDataError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut s = Self::new();
        let payload = s.subscribe(symbols)?;
        Ok((s, payload))
    }

    /// Đăng ký thêm symbols. Trả về payload JSON nếu có symbol mới thực sự được thêm
    pub fn subscribe<I, S>(&mut self, symbols: I) -> Result<Option<String>, MarketDataError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let s = symbols
            .into_iter()
            .map(|x| Self::normalize_symbol(x.as_ref()))
            .collect::<Result<HashSet<String>, MarketDataError>>()?;

        let mut newly_added = Vec::new();
        for sym in s {
            if self.active_symbols.insert(sym.clone()) {
                newly_added.push(sym);
            }
        }

        if newly_added.is_empty() {
            return Ok(None);
        } else {
            newly_added.sort();
            let payload = json!({"type": "subscribe", "symbols": newly_added});
            return Ok(Some(payload.to_string()));
        }
    }

    /// Hủy đăng ký symbols. Trả về payload JSON nếu có symbol thực sự bị xóa
    pub fn unsubscribe<I, S>(&mut self, symbols: I) -> Result<Option<String>, MarketDataError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let s = symbols
            .into_iter()
            .map(|x| Self::normalize_symbol(x.as_ref()))
            .collect::<Result<HashSet<String>, MarketDataError>>()?;

        let mut removed_symbols = Vec::new();
        for sym in s {
            if self.active_symbols.remove(&sym) {
                removed_symbols.push(sym);
            }
        }

        if removed_symbols.is_empty() {
            return Ok(None);
        }

        removed_symbols.sort();

        let payload = json!({"type": "unsubscribe", "symbols": removed_symbols});
        return Ok(Some(payload.to_string()));
    }

    /// Sinh frame subscribe cho toàn bộ symbols hiện có (dùng khi Reconnect)
    pub fn generate_resubscribe_message(&self) -> Result<Option<String>, MarketDataError> {
        let list_symbol = self.get_active_symbols();
        if list_symbol.is_empty() {
            return Ok(None);
        }
        let payload = json!({"type": "subscribe", "symbols": list_symbol});
        return Ok(Some(payload.to_string()));
    }

    /// Kiểm tra một symbol đã được đăng ký hay chưa
    pub fn is_subscribed(&self, symbol: &str) -> bool {
        let normalized = symbol.trim().to_uppercase();
        self.active_symbols.contains(&normalized)
    }

    /// Lấy danh sách symbols đang active (đã sắp xếp để output ổn định)
    pub fn get_active_symbols(&self) -> Vec<String> {
        let mut list_symbols: Vec<String> =
            self.active_symbols.iter().map(|s| s.to_string()).collect();
        list_symbols.sort();
        list_symbols
    }

    /// Helper nội bộ chuẩn hóa ticker (trim + uppercase + kiểm tra rỗng)
    fn normalize_symbol(symbol: &str) -> Result<String, MarketDataError> {
        let trimmed = symbol.trim().to_uppercase();
        if trimmed.is_empty() {
            return Err(MarketDataError::EmptyData(
                "Không thể đăng ký symbol rỗng".to_string(),
            ));
        }
        Ok(trimmed)
    }

    pub fn len(&self) -> usize {
        self.active_symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active_symbols.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_symbol() {
        assert_eq!(
            SubscriptionManager::normalize_symbol(" hpg ").unwrap(),
            "HPG"
        );
        assert_eq!(SubscriptionManager::normalize_symbol("vnm").unwrap(), "VNM");
        assert!(SubscriptionManager::normalize_symbol("").is_err());
        assert!(SubscriptionManager::normalize_symbol("   ").is_err());
    }

    #[test]
    fn test_subscribe_and_deduplication() {
        let mut manager = SubscriptionManager::new();
        let payload = manager.subscribe(["hpg", "vnm"]).unwrap();
        assert!(payload.is_some());
        assert_eq!(manager.len(), 2);
        assert!(manager.is_subscribed("HPG"));
        assert!(manager.is_subscribed("VNM"));

        // Subscribe lại mã đã có -> Trả về None
        let duplicate_payload = manager.subscribe(["HPG"]).unwrap();
        assert_eq!(duplicate_payload, None);
        assert_eq!(manager.len(), 2);
    }

    #[test]
    fn test_unsubscribe() {
        let (mut manager, _) = SubscriptionManager::with_symbols(["HPG", "VNM"]).unwrap();
        let payload = manager.unsubscribe(["HPG"]).unwrap();
        assert!(payload.is_some());
        assert_eq!(manager.len(), 1);
        assert!(!manager.is_subscribed("HPG"));
        assert!(manager.is_subscribed("VNM"));

        // Hủy mã không tồn tại -> Trả về None
        let noop = manager.unsubscribe(["VIC"]).unwrap();
        assert_eq!(noop, None);
    }

    #[test]
    fn test_resubscribe_message() {
        let (manager, _) = SubscriptionManager::with_symbols(["HPG", "VNM"]).unwrap();
        let payload = manager.generate_resubscribe_message().unwrap();
        assert!(payload.is_some());
        let val: serde_json::Value = serde_json::from_str(&payload.unwrap()).unwrap();
        assert_eq!(val["type"], "subscribe");
        assert_eq!(val["symbols"], serde_json::json!(["HPG", "VNM"]));
    }
}
