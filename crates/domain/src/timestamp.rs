use chrono::{DateTime, Datelike, FixedOffset, Utc};

use crate::errors::MarketDataError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MarketTimestamp {
    date_time: DateTime<Utc>,
}

impl MarketTimestamp {
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

    pub fn timestamp_millis(&self) -> i64 {
        self.date_time.timestamp_millis()
    }

    pub fn timestamp_secs(&self) -> i64 {
        self.date_time.timestamp()
    }

    pub fn as_utc(&self) -> DateTime<Utc> {
        self.date_time
    }

    pub fn to_vietnam_time(&self) -> DateTime<FixedOffset> {
        let offset_utc_plus_7 = FixedOffset::east_opt(7 * 3600).unwrap();
        let dt = self.date_time.with_timezone(&offset_utc_plus_7);

        dt
    }

    pub fn from_raw_epoch(epoch: i64) -> Result<Self, MarketDataError> {
        if epoch < 100_000_000_000 {
            return Self::from_epoch_secs(epoch);
        } else {
            return Self::from_epoch_millis(epoch);
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
}
