use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,         // Đang nhận tin tức đều đặn trong ngưỡng
    HeartbeatMissed, // Quá hạn heartbeat nhưng chưa vượt ngưỡng staleness tối đa
    Stale,           // Vượt ngưỡng max_tick_staleness -> Cần kích hoạt Reconnect
    Dead,            // Mất kết nối hoàn toàn hoặc không nhận được frame nào
}

#[derive(Debug)]
pub struct HealthMonitor {
    last_message_ts: AtomicU64,
    last_heartbeat_ts: AtomicU64,
    heartbeat_timeout_ms: u64,
    staleness_timeout_ms: u64,
}

impl HealthMonitor {
    pub fn new(heartbeat_timeout_ms: u64, staleness_timeout_ms: u64) -> Self {
        Self {
            last_message_ts: AtomicU64::new(0),
            last_heartbeat_ts: AtomicU64::new(0),
            heartbeat_timeout_ms,
            staleness_timeout_ms,
        }
    }

    // --- 1. cập nhật atomic ---
    pub fn record_message(&self, current_ts_ms: u64) {
        self.last_message_ts.store(current_ts_ms, Ordering::Relaxed);
    }

    // --- 2.  (cập nhật atomic cả message lẫn heartbeat) ---
    pub fn record_heartbeat(&self, current_ts_ms: u64) {
        self.last_heartbeat_ts
            .store(current_ts_ms, Ordering::Relaxed);

        self.last_message_ts.store(current_ts_ms, Ordering::Relaxed);
    }

    // --- 3. Hàm kiểm tra sức khỏe (gọi định kỳ bởi Task riêng hoặc trong vòng lặp chính) ---
    pub fn check_health(&self, current_time_ms: u64) -> HealthStatus {
        let message_ts = self.last_message_ts.load(Ordering::Relaxed); //10.000

        // false
        if message_ts == 0 {
            return HealthStatus::Dead;
        }

        let message_age = current_time_ms.saturating_sub(message_ts); //12.000 - 10.000 = 2.000

        // 2.000 >= 30.000 false
        if message_age >= self.staleness_timeout_ms {
            return HealthStatus::Stale;
        }

        // 2.000 >= 5.000 && 2.000 < 30.000  false
        if message_age >= self.heartbeat_timeout_ms {
            return HealthStatus::HeartbeatMissed;
        }

        // true
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
