//! Structured logging, metrics collection, and health checks.

use tracing_subscriber::EnvFilter;

/// Khởi tạo hệ thống logging tập trung cho toàn bộ ứng dụng sử dụng `tracing-subscriber`.
///
/// Thiết lập một định dạng subscriber hiển thị log ra stdout, kết hợp lọc theo cấp độ ghi log
/// được truyền vào hoặc ghi đè thông qua biến môi trường `RUST_LOG`.
///
/// # Tham số:
/// - `log_level`: Chuỗi định danh mức độ log mặc định (ví dụ: `"info"`, `"debug"`, `"error"`, `"warn"`, `"trace"`).
///
/// # Cơ chế hoạt động:
/// 1. Kiểm tra biến môi trường `RUST_LOG`, nếu tồn tại sẽ ưu tiên sử dụng.
/// 2. Nếu `RUST_LOG` không được thiết lập, áp dụng mức log từ tham số `log_level`.
/// 3. Khởi tạo global default subscriber bằng `try_init()` để tránh panic nếu hàm được gọi nhiều lần (ví dụ trong unit tests).
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
