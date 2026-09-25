use crate::errors::ConfigError;
use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::convert::AsRef;
use std::path::Path;

/// Cấu hình tổng thể của ứng dụng VN30 Real-Time Analyzer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Cấu hình máy chủ và mức độ ghi log.
    pub server: ServerConfig,
    /// Cấu hình kết nối dữ liệu thị trường (WebSocket).
    pub market_data: MarketDataConfig,
    /// Cấu hình danh mục rổ cổ phiếu cần theo dõi (rổ VN30).
    pub basket: BasketConfig,
    /// Cấu hình bộ lưu trữ trạng thái trong bộ nhớ.
    pub state_store: StateStoreConfig,
    /// Cấu hình trích xuất đặc trưng và tính toán chỉ báo.
    pub features: FeaturesConfig,
    /// Cấu hình mô hình Machine Learning.
    pub model: ModelConfig,
    /// Cấu hình các hồ sơ quản trị rủi ro (safe, medium, risky).
    pub risk: RiskProfilesConfig,
    /// Cấu hình hệ thống cảnh báo qua Telegram bot.
    pub telegram: TelegramConfig,
}

/// Cấu hình máy chủ runtime và môi trường vận hành.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    /// Môi trường vận hành (`"development"`, `"production"`, `"test"`).
    pub environment: String,
    /// Cấp độ ghi nhật ký (`"info"`, `"debug"`, `"error"`, `"warn"`, `"trace"`).
    pub log_level: String,
}

/// Cấu hình kết nối dữ liệu thị trường trực tiếp qua WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketDataConfig {
    /// Định danh nhà cung cấp dữ liệu (`"SSI_FASTCONNECT"`, `"VNDIRECT"`, `"VPS"`, `"SIMULATOR"`).
    pub provider: String,
    /// Địa chỉ endpoint WebSocket của nhà cung cấp (bắt đầu bằng `ws://` hoặc `wss://`).
    pub ws_endpoint: String,
    /// Thời gian chờ khởi đầu khi tái kết nối (mili-giây).
    pub reconnect_initial_backoff_ms: u64,
    /// Thời gian chờ tối đa (trần trễ) khi tái kết nối (mili-giây).
    pub reconnect_max_backoff_ms: u64,
    /// Chu kỳ gửi/nhận nhịp tim duy trì kết nối (giây).
    pub heartbeat_interval_secs: u64,
    /// Ngưỡng thời gian tối đa không có bản tin mới trước khi coi là mất kết nối (giây).
    pub max_tick_staleness_secs: u64,
}

/// Cấu hình danh mục rổ mã cổ phiếu cần theo dõi và phân tích thời gian thực.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BasketConfig {
    /// Thời điểm đồng bộ danh mục hàng ngày theo định dạng `HH:MM:SS` (ví dụ: `"08:00:00"`).
    pub sync_time: String,
    /// Danh sách các mã chứng khoán cần theo dõi (ví dụ: 30 mã rổ VN30).
    pub symbols: Vec<String>,
}

/// Cấu hình bộ nhớ lưu trữ trạng thái rolling-window trong RAM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateStoreConfig {
    /// Kích thước cửa sổ trượt tối đa (số lượng nến hoặc sự kiện tối đa lưu trữ).
    pub rolling_window_size: u64,
}

/// Cấu hình kỹ thuật trích xuất đặc trưng phục vụ Machine Learning và Risk Engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeaturesConfig {
    /// Số lượng thanh nến lookback để tính hệ số Beta thị trường.
    pub beta_lookback_bars: u64,
    /// Số lượng mẫu hợp lệ tối thiểu cần có trước khi tính toán đặc trưng.
    pub min_valid_samples: u64,
}

/// Cấu hình mô hình Machine Learning (Random Forest) và lịch huấn luyện lại.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelConfig {
    /// Đường dẫn tới file artifact nhị phân của mô hình (ví dụ: `"models/rf_active.bin"`).
    pub artifact_path: String,
    /// Lịch trình huấn luyện lại mô hình định kỳ (ví dụ: `"sunday"`).
    pub retrain_schedule: String,
    /// Điểm tin cậy dự báo tối thiểu (trong đoạn `[0.0, 1.0]`) để kích hoạt tín hiệu giao dịch.
    pub min_confidence_score: f32,
}

/// Cấu hình các tham số rủi ro cho một phân hạng hồ sơ cụ thể.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLevelConfig {
    /// Biên lợi nhuận mục tiêu tối thiểu (Take Profit Min).
    pub target_min: f32,
    /// Biên lợi nhuận mục tiêu tối đa (Take Profit Max).
    pub target_max: f32,
    /// Tỷ lệ cắt lỗ tối thiểu (Stop Loss Min, giá trị âm hoặc 0).
    pub sl_min: f32,
    /// Tỷ lệ cắt lỗ tối đa (Stop Loss Max, giá trị âm hoặc 0).
    pub sl_max: f32,
    /// Ngưỡng hệ số Beta tối thiểu cho phép (tùy chọn).
    pub beta_min: Option<f64>,
    /// Ngưỡng hệ số Beta tối đa cho phép (tùy chọn).
    pub beta_max: Option<f64>,
}

/// Cấu hình phân tầng rủi ro gồm 3 mức: An toàn, Trung bình, và Mạo hiểm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskProfilesConfig {
    /// Hồ sơ rủi ro thấp / an toàn (Safe).
    pub safe: RiskLevelConfig,
    /// Hồ sơ rủi ro trung bình (Medium).
    pub medium: RiskLevelConfig,
    /// Hồ sơ rủi ro cao / mạo hiểm (Risky).
    pub risky: RiskLevelConfig,
}

/// Cấu hình kết nối và tần suất phát cảnh báo tới Telegram Bot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelegramConfig {
    /// Tên biến môi trường lưu Token bí mật của Telegram Bot (ví dụ: `"TELEGRAM_BOT_TOKEN"`).
    pub bot_token_env: String,
    /// Danh sách các Chat ID được phép nhận thông báo cảnh báo.
    pub chat_allowlist: Vec<i64>,
    /// Thời gian chờ hạ nhiệt (cooldown/debounce) giữa các cảnh báo cho cùng một mã (phút).
    pub debounce_cooldown_mins: u64,
}

impl AppConfig {
    /// Phân tích cú pháp chuỗi văn bản định dạng TOML thành đối tượng [`AppConfig`].
    ///
    /// # Tham số:
    /// - `content`: Chuỗi nội dung TOML cần giải tuần tự hóa.
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Cấu hình ứng dụng được nạp thành công.
    /// - `Err(ConfigError::ParseError)`: Nếu cú pháp TOML bị sai hoặc thiếu trường dữ liệu bắt buộc.
    pub fn from_str(content: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(content)?;
        Ok(config)
    }

    /// Đọc file cấu hình từ đường dẫn đĩa và giải tuần tự hóa thành [`AppConfig`].
    ///
    /// # Tham số:
    /// - `path`: Đường dẫn tới file cấu hình TOML (ví dụ: `"config/default.toml"`).
    ///
    /// # Giá trị trả về:
    /// - `Ok(Self)`: Cấu hình ứng dụng nạp thành công.
    /// - `Err(ConfigError::IoError)`: Nếu không tìm thấy file hoặc không có quyền đọc.
    /// - `Err(ConfigError::ParseError)`: Nếu nội dung file sai định dạng TOML.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(&path).map_err(|source| ConfigError::IoError {
            path: path.as_ref().display().to_string(),
            source,
        })?;

        Self::from_str(&content)
    }

    /// Thẩm định toàn diện (deep validation) tính hợp lệ logic của toàn bộ các mục cấu hình con.
    ///
    /// Kiểm tra tuần tự theo chuỗi: Server -> MarketData -> Basket -> StateStore -> Features -> Model -> Risk -> Telegram.
    ///
    /// # Giá trị trả về:
    /// - `Ok(())`: Mọi tham số cấu hình đều thỏa mãn các ràng buộc logic.
    /// - `Err(ConfigError::ValidationError)`: Nếu phát hiện bất kỳ giá trị cấu hình nào vi phạm quy tắc.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.server.validate()?;
        self.market_data.validate()?;
        self.basket.validate()?;
        self.state_store.validate()?;
        self.features.validate()?;
        self.model.validate()?;
        self.risk.validate()?;
        self.telegram.validate()?;
        Ok(())
    }
}

impl ServerConfig {
    /// Thẩm định cấu hình máy chủ và môi trường runtime.
    ///
    /// # Quy tắc thẩm định:
    /// - `environment`: Chỉ chấp nhận một trong các giá trị: `"development"`, `"production"`, `"test"`.
    /// - `log_level`: Chỉ chấp nhận một trong các mức: `"info"`, `"debug"`, `"error"`, `"warn"`, `"trace"`.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu môi trường hoặc cấp độ log không hợp lệ.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra environment, log_level
        match self.environment.as_str() {
            "development" | "production" | "test" => {}
            _ => {
                return Err(ConfigError::ValidationError(
                    "Invalid environment".to_string(),
                ));
            }
        }
        match self.log_level.as_str() {
            "info" | "debug" | "error" | "warn" | "trace" => {}
            _ => {
                return Err(ConfigError::ValidationError(
                    "Invalid log level".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl MarketDataConfig {
    /// Thẩm định cấu hình kết nối luồng dữ liệu thị trường (WebSocket).
    ///
    /// # Quy tắc thẩm định:
    /// - `provider`: Phải thuộc danh sách nhà cung cấp hỗ trợ: `"SSI_FASTCONNECT"`, `"VNDIRECT"`, `"VPS"`, `"SIMULATOR"`.
    /// - `ws_endpoint`: Không được rỗng và phải bắt đầu bằng giao thức `"ws://"` hoặc `"wss://"`.
    /// - `reconnect_initial_backoff_ms`: Phải lớn hơn 0.
    /// - `reconnect_max_backoff_ms`: Phải lớn hơn hoặc bằng `reconnect_initial_backoff_ms`.
    /// - `heartbeat_interval_secs` và `max_tick_staleness_secs`: Phải lớn hơn 0.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu bất kỳ thông số kết nối nào vi phạm ràng buộc.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra provider, ws_endpoint, backoff, heartbeat (lưu ý: u64 == 0)

        match self.provider.as_str() {
            "SSI_FASTCONNECT" | "VNDIRECT" | "VPS" | "SIMULATOR" => {}
            _ => {
                return Err(ConfigError::ValidationError("Invalid provider".to_string()));
            }
        }

        match self.ws_endpoint.trim().is_empty() {
            true => {
                return Err(ConfigError::ValidationError(
                    "Invalid ws_endpoint".to_string(),
                ));
            }
            _ => {}
        }

        if !self.ws_endpoint.starts_with("ws://") && !self.ws_endpoint.starts_with("wss://") {
            return Err(ConfigError::ValidationError(
                "ws_endpoint must start with ws:// or wss://".to_string(),
            ));
        }

        match self.reconnect_initial_backoff_ms {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid reconnect_initial_backoff_ms".to_string(),
                ));
            }
            _ => {}
        }

        match self.reconnect_max_backoff_ms {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid reconnect_max_backoff_ms".to_string(),
                ));
            }
            _ => {}
        }

        if self.reconnect_max_backoff_ms < self.reconnect_initial_backoff_ms {
            return Err(ConfigError::ValidationError(
                "reconnect_max_backoff_ms must be greater than or equal to reconnect_initial_backoff_ms".to_string(),
            ));
        }

        match self.heartbeat_interval_secs {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid heartbeat_interval_secs".to_string(),
                ));
            }
            _ => {}
        }

        match self.max_tick_staleness_secs {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid max_tick_staleness_secs".to_string(),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

impl BasketConfig {
    /// Thẩm định cấu hình danh mục rổ cổ phiếu cần theo dõi (ví dụ: rổ VN30).
    ///
    /// # Quy tắc thẩm định:
    /// - `symbols`: Danh sách không được rỗng và không chứa phần tử rỗng / khoảng trắng.
    /// - Các mã cổ phiếu trong danh sách không được trùng lặp.
    /// - `sync_time`: Thời điểm đồng bộ định kỳ hàng ngày phải đúng định dạng giờ:phút:giây (`"%H:%M:%S"`).
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu danh sách mã rỗng, chứa mã trùng lặp, hoặc sai format thời gian.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra symbols.is_empty(), symbol rỗng, duplicate symbol, sync_time format
        if self.symbols.is_empty() {
            return Err(ConfigError::ValidationError(
                "Symbols list cannot be empty".to_string(),
            ));
        }

        match self.symbols.iter().any(|symbol| symbol.trim().is_empty()) {
            true => {
                return Err(ConfigError::ValidationError("Invalid symbol".to_string()));
            }
            false => {}
        }

        let mut unique_symbols: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for symbol in &self.symbols {
            if !unique_symbols.insert(symbol.clone()) {
                return Err(ConfigError::ValidationError(
                    "Duplicate symbols found".to_string(),
                ));
            }
        }

        match NaiveTime::parse_from_str(&self.sync_time, "%H:%M:%S") {
            Ok(_) => {}
            Err(_) => {
                return Err(ConfigError::ValidationError(
                    "Invalid sync_time format".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl StateStoreConfig {
    /// Thẩm định cấu hình bộ lưu trữ trạng thái trong bộ nhớ (State Store).
    ///
    /// # Quy tắc thẩm định:
    /// - `rolling_window_size`: Kích thước cửa sổ trượt (số lượng nến/sự kiện tối đa lưu trong RAM) phải lớn hơn 0.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu `rolling_window_size == 0`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra rolling_window_size == 0
        match self.rolling_window_size {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid rolling_window_size".to_string(),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

impl FeaturesConfig {
    /// Thẩm định cấu hình kỹ thuật trích xuất đặc trưng phục vụ Machine Learning và Risk.
    ///
    /// # Quy tắc thẩm định:
    /// - `beta_lookback_bars`: Số lượng nến hồi quy tính Beta thị trường phải lớn hơn 0.
    /// - `min_valid_samples`: Số mẫu hợp lệ tối thiểu để tính toán chỉ báo/đặc trưng phải lớn hơn 0.
    /// - `min_valid_samples` không được lớn hơn `beta_lookback_bars`.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu các thông số bằng 0 hoặc `min_valid_samples > beta_lookback_bars`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra beta_lookback_bars, min_valid_samples
        match self.beta_lookback_bars {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid beta_lookback_bars".to_string(),
                ));
            }
            _ => {}
        }
        match self.min_valid_samples {
            0 => {
                return Err(ConfigError::ValidationError(
                    "Invalid min_valid_samples".to_string(),
                ));
            }
            _ => {}
        }
        if self.min_valid_samples > self.beta_lookback_bars {
            return Err(ConfigError::ValidationError(
                "min_valid_samples cannot be greater than beta_lookback_bars".to_string(),
            ));
        }

        Ok(())
    }
}

impl ModelConfig {
    /// Thẩm định cấu hình mô hình Machine Learning (Random Forest).
    ///
    /// # Quy tắc thẩm định:
    /// - `artifact_path`: Đường dẫn lưu file mô hình nhị phân không được để trống.
    /// - `retrain_schedule`: Chu kỳ huấn luyện lại (ví dụ cron hoặc ngày trong tuần) không được để trống.
    /// - `min_confidence_score`: Ngưỡng độ tin cậy tối thiểu để kích hoạt tín hiệu phải nằm trong đoạn `[0.0, 1.0]`.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu đường dẫn trống hoặc độ tin cậy nằm ngoài đoạn `[0.0, 1.0]`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra artifact_path, retrain_schedule, min_confidence_score [0.0, 1.0]
        match self.artifact_path.trim().is_empty() {
            true => {
                return Err(ConfigError::ValidationError(
                    "Invalid artifact_path".to_string(),
                ));
            }
            false => {}
        }

        match self.retrain_schedule.trim().is_empty() {
            true => {
                return Err(ConfigError::ValidationError(
                    "Invalid retrain_schedule".to_string(),
                ));
            }
            false => {}
        }

        if !(0.0..=1.0).contains(&self.min_confidence_score) {
            return Err(ConfigError::ValidationError(
                "Invalid min_confidence_score".to_string(),
            ));
        }

        Ok(())
    }
}

impl RiskLevelConfig {
    /// Thẩm định thông số phân bổ rủi ro và ngưỡng Take-Profit / Stop-Loss.
    ///
    /// # Quy tắc thẩm định:
    /// - Lợi nhuận mục tiêu (Take Profit): `target_min > 0.0` và `target_max >= target_min`.
    /// - Giới hạn cắt lỗ (Stop Loss): Phải là số âm/bằng 0, tức `sl_min <= sl_max <= 0.0`.
    /// - Ngưỡng hệ số Beta: Nếu đồng thời có cả `beta_min` và `beta_max` thì bắt buộc `beta_min <= beta_max`.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu vi phạm bất kỳ bất biến kinh tế/tài chính nào nói trên.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra:
        // - target_min > 0.0 && target_max >= target_min
        match self.target_min > 0.0 && self.target_max >= self.target_min {
            true => {}
            false => {
                return Err(ConfigError::ValidationError(
                    "Invalid target_min or target_max".to_string(),
                ));
            }
        }

        // - sl_min <= sl_max && sl_max <= 0.0
        match self.sl_min <= self.sl_max && self.sl_max <= 0.0 {
            true => {}
            false => {
                return Err(ConfigError::ValidationError(
                    "Invalid sl_min or sl_max".to_string(),
                ));
            }
        }
        // - Nếu có cả beta_min và beta_max: beta_min <= beta_max
        match self.beta_min.is_some() && self.beta_max.is_some() {
            true => match self.beta_min.unwrap() <= self.beta_max.unwrap() {
                true => {}
                false => {
                    return Err(ConfigError::ValidationError(
                        "Invalid beta_min or beta_max".to_string(),
                    ));
                }
            },
            false => {}
        }
        Ok(())
    }
}

impl RiskProfilesConfig {
    /// Thẩm định đồng thời cả 3 hồ sơ rủi ro: An toàn (`safe`), Trung bình (`medium`), Mạo hiểm (`risky`).
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu một trong ba hồ sơ cấu hình bị lỗi.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.safe.validate()?;
        self.medium.validate()?;
        self.risky.validate()?;
        Ok(())
    }
}

impl TelegramConfig {
    /// Thẩm định cấu hình cảnh báo Telegram.
    ///
    /// # Quy tắc thẩm định:
    /// - `bot_token_env`: Tên biến môi trường lưu token không được để trống hoặc chỉ chứa khoảng trắng.
    ///
    /// # Lỗi trả về:
    /// - [`ConfigError::ValidationError`]: Nếu tên biến môi trường bị rỗng.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Kiểm tra bot_token_env
        match self.bot_token_env.trim().is_empty() {
            true => {
                return Err(ConfigError::ValidationError(
                    "Invalid bot_token_env".to_string(),
                ));
            }
            false => {}
        }
        Ok(())
    }
}

impl TelegramConfig {
    /// Đọc và lấy giá trị Token Telegram Bot từ biến môi trường hệ thống.
    ///
    /// Tên biến môi trường được chỉ định tại trường `bot_token_env` (ví dụ: `"TELEGRAM_BOT_TOKEN"`).
    ///
    /// # Giá trị trả về:
    /// - `Ok(String)`: Chuỗi bí mật token hợp lệ đã bóc tách từ môi trường.
    /// - `Err(ConfigError::MissingEnvVar)`: Nếu biến môi trường chưa được thiết lập hoặc có giá trị rỗng.
    pub fn load_bot_token(&self) -> Result<String, ConfigError> {
        let bot_token_env = std::env::var(&self.bot_token_env)
            .map_err(|_| ConfigError::MissingEnvVar(self.bot_token_env.clone().to_string()))?;
        if bot_token_env.trim().is_empty() {
            return Err(ConfigError::MissingEnvVar(
                self.bot_token_env.clone().to_string(),
            ));
        }
        Ok(bot_token_env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_valid_app_config() -> AppConfig {
        AppConfig {
            server: ServerConfig {
                environment: "development".to_string(),
                log_level: "info".to_string(),
            },
            market_data: MarketDataConfig {
                provider: "SSI_FASTCONNECT".to_string(),
                ws_endpoint: "wss://api.provider.vn/stream".to_string(),
                reconnect_initial_backoff_ms: 1000,
                reconnect_max_backoff_ms: 30000,
                heartbeat_interval_secs: 15,
                max_tick_staleness_secs: 60,
            },
            basket: BasketConfig {
                sync_time: "08:00:00".to_string(),
                symbols: vec![
                    "ACB".to_string(),
                    "FPT".to_string(),
                    "HPG".to_string(),
                    "VNM".to_string(),
                ],
            },
            state_store: StateStoreConfig {
                rolling_window_size: 500,
            },
            features: FeaturesConfig {
                beta_lookback_bars: 90,
                min_valid_samples: 30,
            },
            model: ModelConfig {
                artifact_path: "models/rf_active.bin".to_string(),
                retrain_schedule: "sunday".to_string(),
                min_confidence_score: 0.75,
            },
            risk: RiskProfilesConfig {
                safe: RiskLevelConfig {
                    target_min: 0.12,
                    target_max: 0.15,
                    sl_min: -0.04,
                    sl_max: -0.03,
                    beta_min: None,
                    beta_max: Some(1.0),
                },
                medium: RiskLevelConfig {
                    target_min: 0.15,
                    target_max: 0.18,
                    sl_min: -0.06,
                    sl_max: -0.05,
                    beta_min: Some(1.0),
                    beta_max: Some(1.25),
                },
                risky: RiskLevelConfig {
                    target_min: 0.20,
                    target_max: 0.25,
                    sl_min: -0.08,
                    sl_max: -0.07,
                    beta_min: Some(1.25),
                    beta_max: None,
                },
            },
            telegram: TelegramConfig {
                bot_token_env: "TELEGRAM_BOT_TOKEN".to_string(),
                chat_allowlist: vec![-100123456789],
                debounce_cooldown_mins: 60,
            },
        }
    }

    // --- Loading & Parsing Tests ---

    #[test]
    fn test_from_file_valid() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/config.example.toml");
        let result = AppConfig::from_file(path);
        assert!(
            result.is_ok(),
            "File configuration invalid: {:?}",
            result.err()
        );
        let config = result.unwrap();
        assert!(
            config.validate().is_ok(),
            "Example config failed validation"
        );
    }

    #[test]
    fn test_from_file_invalid_path() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/config.nonexistent.toml");
        let result = AppConfig::from_file(path);
        assert!(matches!(result, Err(ConfigError::IoError { .. })));
    }

    #[test]
    fn test_invalid_toml_syntax() {
        let toml_str = r#"
        name = "Rust"
        version = "1.80"
        [server
    "#;
        let result = AppConfig::from_str(toml_str);
        assert!(matches!(result, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn test_app_config_full_validate_success() {
        let config = sample_valid_app_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_app_config_validate_propagates_child_error() {
        let mut config = sample_valid_app_config();
        config.server.environment = "invalid_env".to_string();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::ValidationError(_))
        ));
    }

    // --- ServerConfig Validation Tests ---

    #[test]
    fn test_server_config_valid() {
        for env in ["development", "production", "test"] {
            for level in ["info", "debug", "error", "warn", "trace"] {
                let cfg = ServerConfig {
                    environment: env.to_string(),
                    log_level: level.to_string(),
                };
                assert!(
                    cfg.validate().is_ok(),
                    "Failed for env: {env}, level: {level}"
                );
            }
        }
    }

    #[test]
    fn test_server_config_invalid_env() {
        let cfg = ServerConfig {
            environment: "staging".to_string(),
            log_level: "info".to_string(),
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("environment"))
        );
    }

    #[test]
    fn test_server_config_invalid_log_level() {
        let cfg = ServerConfig {
            environment: "development".to_string(),
            log_level: "verbose".to_string(),
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("log level"))
        );
    }

    // --- MarketDataConfig Validation Tests ---

    #[test]
    fn test_market_data_config_valid() {
        for provider in ["SSI_FASTCONNECT", "VNDIRECT", "VPS", "SIMULATOR"] {
            let mut cfg = sample_valid_app_config().market_data;
            cfg.provider = provider.to_string();
            assert!(cfg.validate().is_ok());
        }
    }

    #[test]
    fn test_market_data_invalid_provider() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.provider = "BINANCE".to_string();
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("provider"))
        );
    }

    #[test]
    fn test_market_data_empty_ws_endpoint() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.ws_endpoint = "   ".to_string();
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("ws_endpoint"))
        );
    }

    #[test]
    fn test_market_data_invalid_endpoint_scheme() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.ws_endpoint = "http://api.provider.vn/stream".to_string();
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("ws:// or wss://"))
        );
    }

    #[test]
    fn test_market_data_zero_initial_backoff() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.reconnect_initial_backoff_ms = 0;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("reconnect_initial_backoff_ms"))
        );
    }

    #[test]
    fn test_market_data_zero_max_backoff() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.reconnect_max_backoff_ms = 0;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("reconnect_max_backoff_ms"))
        );
    }

    #[test]
    fn test_market_data_max_backoff_less_than_initial() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.reconnect_initial_backoff_ms = 5000;
        cfg.reconnect_max_backoff_ms = 1000;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("greater than or equal to reconnect_initial_backoff_ms"))
        );
    }

    #[test]
    fn test_market_data_zero_heartbeat() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.heartbeat_interval_secs = 0;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("heartbeat_interval_secs"))
        );
    }

    #[test]
    fn test_market_data_zero_staleness() {
        let mut cfg = sample_valid_app_config().market_data;
        cfg.max_tick_staleness_secs = 0;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("max_tick_staleness_secs"))
        );
    }

    // --- BasketConfig Validation Tests ---

    #[test]
    fn test_basket_config_valid() {
        let cfg = sample_valid_app_config().basket;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_basket_config_empty_symbols() {
        let mut cfg = sample_valid_app_config().basket;
        cfg.symbols = vec![];
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("empty"))
        );
    }

    #[test]
    fn test_basket_config_blank_symbol() {
        let mut cfg = sample_valid_app_config().basket;
        cfg.symbols = vec!["ACB".to_string(), "  ".to_string()];
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("Invalid symbol"))
        );
    }

    #[test]
    fn test_basket_config_duplicate_symbols() {
        let mut cfg = sample_valid_app_config().basket;
        cfg.symbols = vec!["ACB".to_string(), "FPT".to_string(), "ACB".to_string()];
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("Duplicate symbols"))
        );
    }

    #[test]
    fn test_basket_config_invalid_sync_time_format() {
        let mut cfg = sample_valid_app_config().basket;
        for invalid_time in ["08:00", "25:00:00", "invalid_time", "08:60:00"] {
            cfg.sync_time = invalid_time.to_string();
            assert!(
                matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("sync_time")),
                "Failed for {invalid_time}"
            );
        }
    }

    // --- StateStoreConfig Validation Tests ---

    #[test]
    fn test_state_store_config_valid() {
        let cfg = StateStoreConfig {
            rolling_window_size: 500,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_state_store_config_zero_window() {
        let cfg = StateStoreConfig {
            rolling_window_size: 0,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("rolling_window_size"))
        );
    }

    // --- FeaturesConfig Validation Tests ---

    #[test]
    fn test_features_config_valid() {
        let cfg = FeaturesConfig {
            beta_lookback_bars: 90,
            min_valid_samples: 30,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_features_config_min_samples_equal_lookback() {
        let cfg = FeaturesConfig {
            beta_lookback_bars: 60,
            min_valid_samples: 60,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_features_config_zero_beta_lookback() {
        let cfg = FeaturesConfig {
            beta_lookback_bars: 0,
            min_valid_samples: 30,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("beta_lookback_bars"))
        );
    }

    #[test]
    fn test_features_config_zero_min_valid_samples() {
        let cfg = FeaturesConfig {
            beta_lookback_bars: 90,
            min_valid_samples: 0,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("min_valid_samples"))
        );
    }

    #[test]
    fn test_features_config_min_samples_greater_than_lookback() {
        let cfg = FeaturesConfig {
            beta_lookback_bars: 30,
            min_valid_samples: 90,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("greater than beta_lookback_bars"))
        );
    }

    // --- ModelConfig Validation Tests ---

    #[test]
    fn test_model_config_valid() {
        let cfg = ModelConfig {
            artifact_path: "models/rf_active.bin".to_string(),
            retrain_schedule: "sunday".to_string(),
            min_confidence_score: 0.75,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_model_config_empty_artifact_path() {
        let cfg = ModelConfig {
            artifact_path: "   ".to_string(),
            retrain_schedule: "sunday".to_string(),
            min_confidence_score: 0.75,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("artifact_path"))
        );
    }

    #[test]
    fn test_model_config_empty_retrain_schedule() {
        let cfg = ModelConfig {
            artifact_path: "models/rf_active.bin".to_string(),
            retrain_schedule: "  ".to_string(),
            min_confidence_score: 0.75,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("retrain_schedule"))
        );
    }

    #[test]
    fn test_model_config_confidence_score_boundaries() {
        let mut cfg = ModelConfig {
            artifact_path: "models/rf_active.bin".to_string(),
            retrain_schedule: "sunday".to_string(),
            min_confidence_score: 0.0,
        };
        assert!(cfg.validate().is_ok());
        cfg.min_confidence_score = 1.0;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_model_config_invalid_confidence_score() {
        let mut cfg = ModelConfig {
            artifact_path: "models/rf_active.bin".to_string(),
            retrain_schedule: "sunday".to_string(),
            min_confidence_score: -0.01,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("min_confidence_score"))
        );

        cfg.min_confidence_score = 1.01;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("min_confidence_score"))
        );
    }

    // --- RiskLevelConfig & RiskProfilesConfig Validation Tests ---

    #[test]
    fn test_risk_level_config_valid() {
        let cfg = RiskLevelConfig {
            target_min: 0.12,
            target_max: 0.15,
            sl_min: -0.04,
            sl_max: -0.03,
            beta_min: Some(0.8),
            beta_max: Some(1.2),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_risk_level_target_min_non_positive() {
        let mut cfg = RiskLevelConfig {
            target_min: 0.0,
            target_max: 0.15,
            sl_min: -0.04,
            sl_max: -0.03,
            beta_min: None,
            beta_max: None,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("target_min or target_max"))
        );

        cfg.target_min = -0.05;
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("target_min or target_max"))
        );
    }

    #[test]
    fn test_risk_level_target_max_less_than_min() {
        let cfg = RiskLevelConfig {
            target_min: 0.15,
            target_max: 0.10,
            sl_min: -0.04,
            sl_max: -0.03,
            beta_min: None,
            beta_max: None,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("target_min or target_max"))
        );
    }

    #[test]
    fn test_risk_level_sl_positive() {
        let cfg = RiskLevelConfig {
            target_min: 0.10,
            target_max: 0.15,
            sl_min: -0.04,
            sl_max: 0.01,
            beta_min: None,
            beta_max: None,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("sl_min or sl_max"))
        );
    }

    #[test]
    fn test_risk_level_sl_min_greater_than_sl_max() {
        let cfg = RiskLevelConfig {
            target_min: 0.10,
            target_max: 0.15,
            sl_min: -0.02,
            sl_max: -0.05,
            beta_min: None,
            beta_max: None,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("sl_min or sl_max"))
        );
    }

    #[test]
    fn test_risk_level_beta_min_greater_than_beta_max() {
        let cfg = RiskLevelConfig {
            target_min: 0.10,
            target_max: 0.15,
            sl_min: -0.05,
            sl_max: -0.03,
            beta_min: Some(1.5),
            beta_max: Some(1.0),
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("beta_min or beta_max"))
        );
    }

    #[test]
    fn test_risk_profiles_config_valid() {
        let cfg = sample_valid_app_config().risk;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_risk_profiles_invalid_safe_profile() {
        let mut cfg = sample_valid_app_config().risk;
        cfg.safe.target_min = -0.1;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::ValidationError(_))
        ));
    }

    // --- TelegramConfig Validation & Token Loading Tests ---

    #[test]
    fn test_telegram_config_valid() {
        let cfg = TelegramConfig {
            bot_token_env: "TELEGRAM_BOT_TOKEN".to_string(),
            chat_allowlist: vec![123456],
            debounce_cooldown_mins: 60,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_telegram_config_empty_bot_token_env() {
        let cfg = TelegramConfig {
            bot_token_env: "  ".to_string(),
            chat_allowlist: vec![],
            debounce_cooldown_mins: 0,
        };
        assert!(
            matches!(cfg.validate(), Err(ConfigError::ValidationError(msg)) if msg.contains("bot_token_env"))
        );
    }

    #[test]
    fn test_loading_bot_token_valid() {
        let env_key = "TEST_TELEGRAM_BOT_TOKEN_VALID";
        std::env::set_var(env_key, "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11");

        let bot_token = TelegramConfig {
            bot_token_env: String::from(env_key),
            chat_allowlist: vec![],
            debounce_cooldown_mins: 0,
        };
        let result = bot_token.load_bot_token();
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
    }

    #[test]
    fn test_loading_bot_token_missing_env() {
        let bot_token = TelegramConfig {
            bot_token_env: String::from("NON_EXISTENT_TELEGRAM_ENV_KEY_12345"),
            chat_allowlist: vec![],
            debounce_cooldown_mins: 0,
        };
        let result = bot_token.load_bot_token();
        assert!(matches!(result, Err(ConfigError::MissingEnvVar(_))));
    }

    #[test]
    fn test_loading_bot_token_empty() {
        let env_key = "TEST_TELEGRAM_BOT_TOKEN_EMPTY";
        std::env::set_var(env_key, "   ");

        let bot_token = TelegramConfig {
            bot_token_env: String::from(env_key),
            chat_allowlist: vec![],
            debounce_cooldown_mins: 0,
        };
        let result = bot_token.load_bot_token();
        assert!(matches!(result, Err(ConfigError::MissingEnvVar(_))));
    }
}
