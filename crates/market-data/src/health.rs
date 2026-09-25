use std::sync::atomic::{AtomicU64, Ordering};

/// Phân loại trạng thái sức khỏe của kết nối dữ liệu thị trường (Health Status).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Đang nhận tin tức đều đặn trong ngưỡng an toàn (thời gian trễ < heartbeat_timeout).
    Healthy,
    /// Đã quá hạn heartbeat thông thường nhưng chưa vượt ngưỡng staleness tối đa.
    HeartbeatMissed,
    /// Vượt ngưỡng staleness tối đa -> Dữ liệu đã bị đông cứng, cần kích hoạt Reconnect khẩn cấp.
    Stale,
    /// Mất kết nối hoàn toàn hoặc từ lúc khởi động chưa từng nhận được bất kỳ frame nào.
    Dead,
}

/// Bộ giám sát sức khỏe kết nối phi khóa (Lock-free Health Monitor) sử dụng `AtomicU64`.
///
/// Cho phép cập nhật timestamp từ Reader task song song với việc kiểm tra định kỳ từ Task giám sát
/// mà không xảy ra tranh chấp khóa (contention-free).
#[derive(Debug)]
pub struct HealthMonitor {
    last_message_ts: AtomicU64,
    last_heartbeat_ts: AtomicU64,
    heartbeat_timeout_ms: u64,
    staleness_timeout_ms: u64,
}

impl HealthMonitor {
    /// Khởi tạo một bộ giám sát sức khỏe mới với các ngưỡng thời gian tính bằng mili-giây.
    ///
    /// # Tham số:
    /// - `heartbeat_timeout_ms`: Ngưỡng thời gian cảnh báo mất heartbeat (ví dụ: 5.000ms).
    /// - `staleness_timeout_ms`: Ngưỡng thời gian tối đa không có dữ liệu trước khi coi là mất kết nối (ví dụ: 30.000ms).
    ///
    /// # Giá trị trả về:
    /// - `Self`: Instance với các mốc thời gian ban đầu bằng 0 (trạng thái ban đầu là `Dead`).
    pub fn new(heartbeat_timeout_ms: u64, staleness_timeout_ms: u64) -> Self {
        Self {
            last_message_ts: AtomicU64::new(0),
            last_heartbeat_ts: AtomicU64::new(0),
            heartbeat_timeout_ms,
            staleness_timeout_ms,
        }
    }

    /// Ghi nhận mốc thời gian nhận được bản tin dữ liệu thị trường bất kỳ (Trade/Quote/Ping...).
    ///
    /// Cập nhật nguyên tử mốc `last_message_ts` bằng `Ordering::Relaxed`.
    ///
    /// # Tham số:
    /// - `current_ts_ms`: Thời điểm hiện tại tính bằng Unix epoch mili-giây.
    pub fn record_message(&self, current_ts_ms: u64) {
        self.last_message_ts.store(current_ts_ms, Ordering::Relaxed);
    }

    /// Ghi nhận mốc thời gian nhận được tín hiệu Heartbeat từ máy chủ sàn.
    ///
    /// Đồng thời cập nhật cả `last_heartbeat_ts` và `last_message_ts`.
    ///
    /// # Tham số:
    /// - `current_ts_ms`: Thời điểm nhận tín hiệu nhịp tim tính bằng Unix epoch mili-giây.
    pub fn record_heartbeat(&self, current_ts_ms: u64) {
        self.last_heartbeat_ts
            .store(current_ts_ms, Ordering::Relaxed);

        self.last_message_ts.store(current_ts_ms, Ordering::Relaxed);
    }

    /// Thẩm định trạng thái sức khỏe hiện tại của kết nối đối chiếu với mốc thời gian `current_time_ms`.
    ///
    /// # Quy tắc phân loại:
    /// 1. Nếu chưa từng nhận bản tin nào (`last_message_ts == 0`): Trả về [`HealthStatus::Dead`].
    /// 2. Nếu độ trễ `>= staleness_timeout_ms`: Trả về [`HealthStatus::Stale`] (kích hoạt reconnect).
    /// 3. Nếu độ trễ `>= heartbeat_timeout_ms`: Trả về [`HealthStatus::HeartbeatMissed`].
    /// 4. Ngược lại: Trả về [`HealthStatus::Healthy`].
    ///
    /// # Tham số:
    /// - `current_time_ms`: Thời điểm hiện tại tính bằng Unix epoch mili-giây.
    ///
    /// # Giá trị trả về:
    /// - [`HealthStatus`]: Trạng thái đánh giá tại thời điểm kiểm tra.
    pub fn check_health(&self, current_time_ms: u64) -> HealthStatus {
        let message_ts = self.last_message_ts.load(Ordering::Relaxed);

        if message_ts == 0 {
            return HealthStatus::Dead;
        }

        let message_age = current_time_ms.saturating_sub(message_ts);

        if message_age >= self.staleness_timeout_ms {
            return HealthStatus::Stale;
        }

        if message_age >= self.heartbeat_timeout_ms {
            return HealthStatus::HeartbeatMissed;
        }

        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_health_initial_state_is_dead() {
        let monitor = HealthMonitor::new(5_000, 30_000);
        assert_eq!(monitor.check_health(10_000), HealthStatus::Dead);
    }

    #[test]
    fn test_health_record_message_healthy() {
        let monitor = HealthMonitor::new(5_000, 30_000);
        monitor.record_message(10_000);

        // Kiểm tra sau 2 giây (trong hạn heartbeat 5s) -> Healthy
        assert_eq!(monitor.check_health(12_000), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_record_heartbeat_healthy() {
        let monitor = HealthMonitor::new(5_000, 30_000);
        monitor.record_heartbeat(10_000);

        // Kiểm tra sau 3 giây -> Healthy
        assert_eq!(monitor.check_health(13_000), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_heartbeat_missed() {
        let monitor = HealthMonitor::new(5_000, 30_000);
        monitor.record_message(10_000);

        // Sau 6 giây (vượt quá heartbeat 5s, nhưng chưa tới staleness 30s) -> HeartbeatMissed
        assert_eq!(monitor.check_health(16_000), HealthStatus::HeartbeatMissed);
    }

    #[test]
    fn test_health_stale_triggers_timeout() {
        let monitor = HealthMonitor::new(5_000, 30_000);
        monitor.record_message(10_000);

        // Sau 35 giây (vượt quá staleness 30s) -> Stale
        assert_eq!(monitor.check_health(45_000), HealthStatus::Stale);
    }

    #[test]
    fn test_health_concurrency_multi_threaded() {
        let monitor = Arc::new(HealthMonitor::new(5_000, 30_000));
        let mut handles = Vec::new();

        // Spawn 8 threads cập nhật message liên tục
        for i in 0..8 {
            let mon = Arc::clone(&monitor);
            let handle = thread::spawn(move || {
                for step in 0..100 {
                    let ts = 10_000 + (i * 100) + step;
                    mon.record_message(ts);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread should finish successfully");
        }

        // Sau khi tất cả threads ghi xong, kiểm tra health ở thời điểm gần nhất
        let last_ts = 10_000 + (7 * 100) + 99;
        assert_eq!(monitor.check_health(last_ts + 1_000), HealthStatus::Healthy);
    }
}
