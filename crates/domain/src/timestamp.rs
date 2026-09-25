use chrono::{DateTime, Datelike, Duration, FixedOffset, Utc};

use crate::errors::MarketDataError;
use serde::{Deserialize, Serialize};

/// Kiểu dữ liệu Newtype đóng gói mốc thời gian chuẩn hóa (`DateTime<Utc>`) của thị trường chứng khoán.
///
/// Duy trì bất biến (domain invariant):
/// - Timestamp phải là số dương (`timestamp > 0`).
/// - Năm phải nằm trong khoảng hợp lệ từ 2000 đến 2100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MarketTimestamp {
    date_time: DateTime<Utc>,
}

impl MarketTimestamp {
    /// Khởi tạo `MarketTimestamp` từ mốc thời gian Unix epoch tính bằng mili-giây (milliseconds).
    ///
    /// # Tham số:
    /// - `ms`: Số mili-giây tính từ Unix epoch (1970-01-01T00:00:00Z).
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Mốc thời gian hợp lệ thỏa mãn điều kiện năm trong khoảng 2000-2100.
    /// - `Err(MarketDataError::InvalidTimestamp)`: Nếu giá trị ms vượt quá giới hạn biểu diễn thời gian hoặc không thỏa bất biến.
    pub fn from_epoch_millis(ms: i64) -> Result<Self, MarketDataError> {
        if let Some(dt) = DateTime::from_timestamp_millis(ms) {
            let new_ts = Self::from_utc(dt);
            return new_ts;
        } else {
            return Err(MarketDataError::InvalidTimestamp(
                "Invalid timestamp".to_string(),
            ));
        }
    }

    /// Khởi tạo `MarketTimestamp` từ mốc thời gian Unix epoch tính bằng giây (seconds).
    ///
    /// # Tham số:
    /// - `secs`: Số giây tính từ Unix epoch (1970-01-01T00:00:00Z).
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Mốc thời gian hợp lệ thỏa mãn điều kiện năm trong khoảng 2000-2100.
    /// - `Err(MarketDataError::InvalidTimestamp)`: Nếu giá trị secs vượt quá giới hạn biểu diễn thời gian hoặc không thỏa bất biến.
    pub fn from_epoch_secs(secs: i64) -> Result<Self, MarketDataError> {
        if let Some(dt) = DateTime::from_timestamp_secs(secs) {
            let new_ts = Self::from_utc(dt);
            return new_ts;
        } else {
            return Err(MarketDataError::InvalidTimestamp(
                "Invalid timestamp".to_string(),
            ));
        }
    }

    /// Khởi tạo `MarketTimestamp` từ đối tượng `DateTime<Utc>`, thẩm định các bất biến cốt lõi.
    ///
    /// # Tham số:
    /// - `dt`: Thời điểm UTC cần đóng gói.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Nếu `dt.timestamp() > 0` và năm nằm trong khoảng [2000, 2100].
    /// - `Err(MarketDataError::InvalidTimestamp)`: Nếu vi phạm bất biến về năm hoặc mốc thời gian âm/bằng 0.
    pub fn from_utc(dt: DateTime<Utc>) -> Result<Self, MarketDataError> {
        if dt.timestamp() <= 0 {
            return Err(MarketDataError::InvalidTimestamp(
                "Invalid timestamp".to_string(),
            ));
        } else if dt.year() < 2000 || dt.year() > 2100 {
            return Err(MarketDataError::InvalidTimestamp(
                "Invalid timestamp".to_string(),
            ));
        }
        return Ok(Self { date_time: dt });
    }

    /// Lấy giá trị Unix epoch tính theo mili-giây (milliseconds).
    ///
    /// # Giá trị trả về:
    /// - `i64`: Số mili-giây tính từ mốc 1970-01-01T00:00:00Z.
    pub fn timestamp_millis(&self) -> i64 {
        self.date_time.timestamp_millis()
    }

    /// Lấy giá trị Unix epoch tính theo giây (seconds).
    ///
    /// # Giá trị trả về:
    /// - `i64`: Số giây tính từ mốc 1970-01-01T00:00:00Z.
    pub fn timestamp_secs(&self) -> i64 {
        self.date_time.timestamp()
    }

    /// Lấy tham chiếu bản sao của đối tượng `DateTime<Utc>` nguyên bản.
    ///
    /// # Giá trị trả về:
    /// - `DateTime<Utc>`: Thời điểm được biểu diễn theo chuẩn UTC.
    pub fn as_utc(&self) -> DateTime<Utc> {
        self.date_time
    }

    /// Chuyển đổi mốc thời gian UTC sang múi giờ Việt Nam (ICT, UTC+7).
    ///
    /// Hỗ trợ hiển thị đúng phiên giao dịch chứng khoán Việt Nam (HOSE/HNX) trong log hoặc bản tin Telegram.
    ///
    /// # Giá trị trả về:
    /// - `DateTime<FixedOffset>`: Thời điểm với độ lệch múi giờ +07:00.
    pub fn to_vietnam_time(&self) -> DateTime<FixedOffset> {
        let offset_utc_plus_7 = FixedOffset::east_opt(7 * 3600).unwrap();
        let dt = self.date_time.with_timezone(&offset_utc_plus_7);

        dt
    }

    /// Tự động nhận diện và chuyển đổi epoch số nguyên thô (giây hoặc mili-giây) sang `MarketTimestamp`.
    ///
    /// # Cơ chế:
    /// - Nếu `epoch < 100_000_000_000`: Coi là mốc thời gian theo đơn vị giây (`secs`).
    /// - Ngược lại: Coi là mốc thời gian theo đơn vị mili-giây (`ms`).
    ///
    /// # Tham số:
    /// - `epoch`: Số nguyên biểu diễn thời gian nhận được từ bản tin thô của sàn giao dịch.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: `MarketTimestamp` được chuẩn hóa thành công.
    /// - `Err(MarketDataError::InvalidTimestamp)`: Nếu giá trị không hợp lệ theo bất biến hệ thống.
    pub fn from_raw_epoch(epoch: i64) -> Result<Self, MarketDataError> {
        if epoch < 100_000_000_000 {
            return Self::from_epoch_secs(epoch);
        } else {
            return Self::from_epoch_millis(epoch);
        }
    }

    /// Trừ an toàn một khoảng thời gian (`std::time::Duration`) khỏi mốc thời gian hiện tại.
    ///
    /// Thường được sử dụng để tính toán Watermark (mốc thời gian bắt đầu xả sự kiện)
    /// bằng cách lấy `max_seen_ts - allowed_lateness`.
    ///
    /// # Tham số:
    /// - `duration`: Khoảng thời gian cho phép trễ cần trừ đi.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Mốc thời gian mới sau khi trừ, vẫn đảm bảo bất biến (năm 2000-2100).
    /// - `Err(MarketDataError::InvalidTimestamp)`: Nếu phép trừ bị tràn số hoặc rơi ra ngoài biên hợp lệ.
    pub fn checked_sub_duration(
        &self,
        duration: std::time::Duration,
    ) -> Result<Self, MarketDataError> {
        let new_time = Duration::from_std(duration)
            .map_err(|e| MarketDataError::InvalidTimestamp(e.to_string()))?;
        if let Some(new_dt) = self.date_time.checked_sub_signed(new_time) {
            return Self::from_utc(new_dt);
        } else {
            return Err(MarketDataError::InvalidTimestamp(
                "Invalid timestamp".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_valid_from_epoch_millis() {
        // 2024-09-05T10:00:00Z -> 1725530400000 ms
        let ms = 1_725_530_400_000;
        let ts = MarketTimestamp::from_epoch_millis(ms).expect("timestamp millis should be valid");

        assert_eq!(ts.timestamp_millis(), ms);
        assert_eq!(ts.timestamp_secs(), 1_725_530_400);
        assert_eq!(ts.as_utc().year(), 2024);
    }

    #[test]
    fn test_valid_from_epoch_secs() {
        let secs = 1_725_530_400;
        let ts = MarketTimestamp::from_epoch_secs(secs).expect("timestamp secs should be valid");

        assert_eq!(ts.timestamp_secs(), secs);
        assert_eq!(ts.timestamp_millis(), secs * 1000);
    }

    #[test]
    fn test_reject_negative_or_zero_timestamp() {
        assert!(MarketTimestamp::from_epoch_millis(-1).is_err());
        assert!(MarketTimestamp::from_epoch_millis(0).is_err());
        assert!(MarketTimestamp::from_epoch_secs(-100).is_err());
        assert!(MarketTimestamp::from_epoch_secs(0).is_err());
    }

    #[test]
    fn test_reject_year_before_2000() {
        // Year 1970 UNIX epoch (1000 ms)
        assert!(MarketTimestamp::from_epoch_millis(1000).is_err());

        // 1999-12-31T23:59:59Z -> 946684799000 ms
        let pre_2000_ms = 946_684_799_000;
        assert!(MarketTimestamp::from_epoch_millis(pre_2000_ms).is_err());
    }

    #[test]
    fn test_reject_year_after_2100() {
        // 2101-01-01T00:00:00Z -> 4133980800000 ms
        let post_2100_ms = 4_133_980_800_000;
        assert!(MarketTimestamp::from_epoch_millis(post_2100_ms).is_err());
    }

    #[test]
    fn test_to_vietnam_time_conversion() {
        // 2026-09-05 02:15:30 UTC -> 2026-09-05 09:15:30 (UTC+7)
        let utc_dt = DateTime::from_timestamp_millis(1_788_574_530_000).unwrap();
        let ts = MarketTimestamp::from_utc(utc_dt).unwrap();

        let vn_dt = ts.to_vietnam_time();
        assert_eq!(vn_dt.hour(), (ts.as_utc().hour() + 7) % 24);
        assert_eq!(vn_dt.minute(), ts.as_utc().minute());
        assert_eq!(vn_dt.second(), ts.as_utc().second());
    }

    #[test]
    fn test_ordering_and_equality() {
        let t1 = MarketTimestamp::from_epoch_millis(1_725_530_400_000).unwrap();
        let t2 = MarketTimestamp::from_epoch_millis(1_725_530_401_000).unwrap();

        assert!(t1 < t2);
        assert_eq!(t1, t1);
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_serde_json_roundtrip() {
        let ts = MarketTimestamp::from_epoch_millis(1_725_530_400_000).unwrap();
        let json = serde_json::to_string(&ts).expect("serialize should succeed");
        let deserialized: MarketTimestamp =
            serde_json::from_str(&json).expect("deserialize should succeed");

        assert_eq!(ts, deserialized);
    }

    #[test]
    fn test_checked_sub_duration_success() {
        let base_ms = 1_725_530_400_000; // 2024-09-05T10:00:00Z
        let ts = MarketTimestamp::from_epoch_millis(base_ms).unwrap();

        let sub_dur = std::time::Duration::from_millis(2500);
        let result = ts.checked_sub_duration(sub_dur).expect("should succeed");

        assert_eq!(result.timestamp_millis(), base_ms - 2500);
        assert_eq!(result.timestamp_secs(), (base_ms - 2500) / 1000);
    }

    #[test]
    fn test_checked_sub_duration_underflow() {
        let base_ms = 1_725_530_400_000;
        let ts = MarketTimestamp::from_epoch_millis(base_ms).unwrap();

        // 30 years in seconds -> brings timestamp before year 2000
        let thirty_years = std::time::Duration::from_secs(30 * 365 * 24 * 3600);
        let result = ts.checked_sub_duration(thirty_years);

        assert!(result.is_err());
    }
}
