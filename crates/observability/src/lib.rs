//! Structured logging, metrics collection, and health checks.

use std::str::FromStr;
use tracing_subscriber::{fmt, EnvFilter};

/// Khởi tạo hệ thống logging với tracing subscriber
pub fn init_logging(log_level: &str) {
    // 1. Tạo EnvFilter từ log_level hoặc RUST_LOG nếu có
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // 2. Khởi tạo tracing_subscriber::fmt
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();

    // 3. Cài đặt làm default/global subscriber
    tracing::debug!("Logging initialized with level: {}", log_level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging() {
        init_logging("error");
        tracing::error!("Test log");
    }
}
