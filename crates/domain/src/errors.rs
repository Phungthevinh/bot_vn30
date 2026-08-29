use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Không thể đọc file cấu hình tại '{path}': {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Lỗi cú pháp TOML trong cấu hình: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Dữ liệu cấu hình không hợp lệ: {0}")]
    ValidationError(String),
    #[error("Biến môi trường bắt buộc '{0}' chưa được thiết lập")]
    MissingEnvVar(String),
}

#[derive(Error, Debug)]
pub enum MarketDataError {
    #[error("Lỗi kết nối WebSocket: {0}")]
    ConnectionError(String),
    #[error("Lỗi phân tích cú pháp bản tin thị trường: {0}")]
    ParseError(String),
    #[error("Dữ liệu thị trường không hợp lệ cho mã '{symbol}': {reason}")]
    InvalidData { symbol: String, reason: String },
    #[error("Dữ liệu thị trường bị quá hạn (stale): timestamp={0}")]
    StaleData(i64),
    #[error("Lỗi xác thực: {0}")]
    AuthenticationError(String),
    #[error("dữ liệu rỗng: {0}")]
    EmptyData(String),
}

#[derive(Error, Debug)]
pub enum IndicatorError {
    #[error("Chưa đủ dữ liệu warm-up cho chỉ báo '{name}': yêu cầu {required}, hiện có {actual}")]
    InsufficientData {
        name: String,
        required: usize,
        actual: usize,
    },
    #[error("Giá trị tính toán không hợp lệ (NaN/Inf) trong chỉ báo '{0}'")]
    CalculationError(String),
    #[error("Cấu hình tham số chỉ báo không hợp lệ: {0}")]
    InvalidParameter(String),
}

#[derive(Error, Debug)]
pub enum RiskError {
    #[error("Dữ liệu nến không đủ để tính Beta/Risk: {0}")]
    InsufficientData(String),
    #[error("Giá trị Benchmark VN30 không hợp lệ: {0}")]
    InvalidBenchmark(String),
    #[error("Vi phạm quy tắc quản trị rủi ro: {0}")]
    RuleViolation(String),
}

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("Không thể sinh tín hiệu do thiếu dữ liệu: {0}")]
    MissingInput(String),
    #[error("Xung đột điều kiện tín hiệu: {0}")]
    Conflict(String),
}

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Không thể tải model artifact từ '{path}': {reason}")]
    LoadError { path: String, reason: String },
    #[error("Kích thước vector đặc trưng không khớp: yêu cầu {expected}, nhận {actual}")]
    FeatureMismatch { expected: usize, actual: usize },
    #[error("Lỗi thực thi suy luận mô hình (inference): {0}")]
    InferenceError(String),
    #[error("Phiên bản mô hình '{0}' không tương thích")]
    IncompatibleVersion(String),
}

#[derive(Error, Debug)]
pub enum AlertError {
    #[error("Lỗi gửi tin nhắn Telegram: {0}")]
    DeliveryFailed(String),
    #[error("Vượt quá giới hạn tần suất gửi tin (Rate limited): thử lại sau {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("Lỗi định dạng bản tin cảnh báo: {0}")]
    FormattingError(String),
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Lỗi cấu hình: {0}")]
    Config(#[from] ConfigError),

    #[error("Lỗi dữ liệu thị trường: {0}")]
    MarketData(#[from] MarketDataError),

    #[error("Lỗi tính toán chỉ báo: {0}")]
    Indicator(#[from] IndicatorError),

    #[error("Lỗi quản trị rủi ro: {0}")]
    Risk(#[from] RiskError),

    #[error("Lỗi tín hiệu: {0}")]
    Signal(#[from] SignalError),

    #[error("Lỗi mô hình ML: {0}")]
    Model(#[from] ModelError),

    #[error("Lỗi cảnh báo: {0}")]
    Alert(#[from] AlertError),

    #[error("Lỗi dữ liệu chung: {0}")]
    InvalidData(String),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_domain_error_from_conversions() {
        let mkt_err = MarketDataError::StaleData(1700000000);
        let domain_err: DomainError = mkt_err.into();
        assert!(matches!(domain_err, DomainError::MarketData(_)));

        let ind_err = IndicatorError::CalculationError("NaN".to_string());
        let domain_err: DomainError = ind_err.into();
        assert!(matches!(domain_err, DomainError::Indicator(_)));

        let risk_err = RiskError::InsufficientData("100 candles needed".to_string());
        let domain_err: DomainError = risk_err.into();
        assert!(matches!(domain_err, DomainError::Risk(_)));

        let sig_err = SignalError::Conflict("Long & Short signals".to_string());
        let domain_err: DomainError = sig_err.into();
        assert!(matches!(domain_err, DomainError::Signal(_)));

        let modl_err = ModelError::InferenceError("Bad weights".to_string());
        let domain_err: DomainError = modl_err.into();
        assert!(matches!(domain_err, DomainError::Model(_)));

        let alert_err = AlertError::DeliveryFailed("Telegram down".to_string());
        let domain_err: DomainError = alert_err.into();
        assert!(matches!(domain_err, DomainError::Alert(_)));
    }

    #[test]
    fn test_error_display_messages() {
        let err = AlertError::RateLimited {
            retry_after_secs: 100,
        };
        assert_eq!(
            err.to_string(),
            "Vượt quá giới hạn tần suất gửi tin (Rate limited): thử lại sau 100s"
        );
    }
}
