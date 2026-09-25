use serde_json::json;
use std::collections::HashSet;
use vn30_domain::errors::MarketDataError;

/// Bộ quản lý danh sách đăng ký nhận dữ liệu (Subscription Manager) cho luồng WebSocket.
///
/// Hỗ trợ:
/// - Đăng ký (`subscribe`) và hủy đăng ký (`unsubscribe`) động trong lúc ứng dụng đang chạy.
/// - Khử trùng lặp mã (deduplication) để không gửi thừa các frame yêu cầu lên sàn.
/// - Tự động tái tạo frame đăng ký tổng thể ([`Self::generate_resubscribe_message`]) khi xảy ra sự kiện Reconnect.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionManager {
    active_symbols: HashSet<String>,
}

impl SubscriptionManager {
    /// Khởi tạo một trình quản lý đăng ký rỗng, chưa theo dõi bất kỳ mã nào.
    ///
    /// # Giá trị trả về:
    /// - `Self`: Instance mới với danh sách mã rỗng.
    pub fn new() -> Self {
        Self {
            active_symbols: HashSet::new(),
        }
    }

    /// Khởi tạo trình quản lý với một danh sách các mã chứng khoán ban đầu.
    ///
    /// # Tham số:
    /// - `symbols`: Tập hợp các mã cổ phiếu / hợp đồng tương lai ban đầu (ví dụ: rổ VN30).
    ///
    /// # Giá trị trả về:
    /// - `Ok((Self, Option<String>))`: Bộ đôi gồm instance vừa tạo và chuỗi JSON subscribe payload
    ///   (nếu có mã hợp lệ) sẵn sàng gửi trong frame bắt tay đầu tiên.
    /// - `Err(MarketDataError::EmptyData)`: Nếu danh sách chứa mã rỗng không hợp lệ.
    pub fn with_symbols<I, S>(symbols: I) -> Result<(Self, Option<String>), MarketDataError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut s = Self::new();
        let payload = s.subscribe(symbols)?;
        Ok((s, payload))
    }

    /// Đăng ký thêm các mã chứng khoán vào danh sách theo dõi.
    ///
    /// Tự động chuẩn hóa viết hoa và loại bỏ các mã đã tồn tại sẵn trong danh sách.
    ///
    /// # Tham số:
    /// - `symbols`: Danh sách mã muốn đăng ký thêm.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Some(String))`: Chuỗi JSON payload đăng ký chứa danh sách các mã MỚI thực sự được thêm.
    /// - `Ok(None)`: Nếu toàn bộ các mã yêu cầu đều đã nằm trong danh sách theo dõi từ trước.
    /// - `Err(MarketDataError::EmptyData)`: Nếu phát hiện mã rỗng hoặc chỉ chứa khoảng trắng.
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

    /// Hủy đăng ký nhận dữ liệu cho một tập hợp các mã chứng khoán.
    ///
    /// # Tham số:
    /// - `symbols`: Danh sách mã muốn ngừng theo dõi.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Some(String))`: Chuỗi JSON payload hủy đăng ký chứa danh sách các mã THỰC SỰ bị xóa.
    /// - `Ok(None)`: Nếu không có mã nào trong yêu cầu từng nằm trong danh sách theo dõi.
    /// - `Err(MarketDataError::EmptyData)`: Nếu phát hiện mã rỗng trong yêu cầu.
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

    /// Sinh bản tin đăng ký (Subscribe frame) tổng thể cho TOÀN BỘ các mã đang được kích hoạt.
    ///
    /// Được gọi tự động trong quy trình tái kết nối (Reconnect) sau khi bắt tay xác thực thành công,
    /// nhằm đảm bảo không bị mất luồng dữ liệu của rổ cổ phiếu đang phân tích.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Some(String))`: Chuỗi JSON frame subscribe chứa toàn bộ active symbols.
    /// - `Ok(None)`: Nếu danh sách theo dõi hiện đang trống rỗng.
    pub fn generate_resubscribe_message(&self) -> Result<Option<String>, MarketDataError> {
        let list_symbol = self.get_active_symbols();
        if list_symbol.is_empty() {
            return Ok(None);
        }
        let payload = json!({"type": "subscribe", "symbols": list_symbol});
        return Ok(Some(payload.to_string()));
    }

    /// Kiểm tra xem một mã chứng khoán hiện có đang nằm trong danh sách theo dõi hay không.
    ///
    /// # Tham số:
    /// - `symbol`: Mã cần kiểm tra (không phân biệt chữ hoa, chữ thường và khoảng trắng).
    ///
    /// # Giá trị trả về:
    /// - `true`: Mã đang được đăng ký.
    /// - `false`: Mã không nằm trong danh sách theo dõi.
    pub fn is_subscribed(&self, symbol: &str) -> bool {
        let normalized = symbol.trim().to_uppercase();
        self.active_symbols.contains(&normalized)
    }

    /// Lấy toàn bộ danh sách các mã cổ phiếu đang được theo dõi, đã được sắp xếp tăng dần.
    ///
    /// Việc sắp xếp thứ tự bảng chữ cái giúp output ổn định phục vụ serialization và logging.
    ///
    /// # Giá trị trả về:
    /// - `Vec<String>`: Vector chứa các mã chứng khoán chuẩn hóa.
    pub fn get_active_symbols(&self) -> Vec<String> {
        let mut list_symbols: Vec<String> =
            self.active_symbols.iter().map(|s| s.to_string()).collect();
        list_symbols.sort();
        list_symbols
    }

    /// Hàm trợ giúp nội bộ: Chuẩn hóa mã chứng khoán (cắt tỉa khoảng trắng, chuyển chữ hoa)
    /// và kiểm tra tính hợp lệ không rỗng.
    ///
    /// # Tham số:
    /// - `symbol`: Chuỗi mã thô.
    ///
    /// # Lỗi trả về:
    /// - [`MarketDataError::EmptyData`]: Nếu chuỗi sau khi trim bị rỗng.
    fn normalize_symbol(symbol: &str) -> Result<String, MarketDataError> {
        let trimmed = symbol.trim().to_uppercase();
        if trimmed.is_empty() {
            return Err(MarketDataError::EmptyData(
                "Không thể đăng ký symbol rỗng".to_string(),
            ));
        }
        Ok(trimmed)
    }

    /// Trả về tổng số lượng mã chứng khoán hiện đang được đăng ký theo dõi.
    pub fn len(&self) -> usize {
        self.active_symbols.len()
    }

    /// Kiểm tra xem danh sách theo dõi có đang trống hay không.
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
