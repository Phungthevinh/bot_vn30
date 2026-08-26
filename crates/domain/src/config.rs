use crate::errors::ConfigError;
use serde::{Deserialize, Serialize};
use std::convert::AsRef;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub market_data: MarketDataConfig,
    pub basket: BasketConfig,
    pub state_store: StateStoreConfig,
    pub features: FeaturesConfig,
    pub model: ModelConfig,
    pub risk: RiskProfilesConfig,
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    pub environment: String,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketDataConfig {
    pub provider: String,
    pub ws_endpoint: String,
    pub reconnect_initial_backoff_ms: u64,
    pub reconnect_max_backoff_ms: u64,
    pub heartbeat_interval_secs: u64,
    pub max_tick_staleness_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BasketConfig {
    pub sync_time: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateStoreConfig {
    pub rolling_window_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeaturesConfig {
    pub beta_lookback_bars: u64,
    pub min_valid_samples: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelConfig {
    pub artifact_path: String,
    pub retrain_schedule: String,
    pub min_confidence_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLevelConfig {
    pub target_min: f32,
    pub target_max: f32,
    pub sl_min: f32,
    pub sl_max: f32,
    pub beta_min: Option<f64>,
    pub beta_max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskProfilesConfig {
    pub safe: RiskLevelConfig,
    pub medium: RiskLevelConfig,
    pub risky: RiskLevelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelegramConfig {
    pub bot_token_env: String,
    pub chat_allowlist: Vec<i64>,
    pub debounce_cooldown_mins: u64,
}

impl AppConfig {
    pub fn from_str(content: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(content)?;
        Ok(config)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(&path).map_err(|source| ConfigError::IoError {
            path: path.as_ref().display().to_string(),
            source,
        })?;

        Self::from_str(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_file_vaild() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/config.example.toml");
        let result = AppConfig::from_file(path);
        assert!(
            result.is_ok(),
            "File configuration invalid: {}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_from_file_invalid() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/config.invalid.toml");
        let result = AppConfig::from_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_toml() {
        let toml_str = r#"
        name = "Rust"
        version = "1.80"
    "#;
        let result = AppConfig::from_str(toml_str);
        assert!(result.is_err());
    }
}
