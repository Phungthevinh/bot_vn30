use std::collections::BTreeMap;
use std::mem;

use std::time::Duration;
use vn30_domain::market::MarketEvent;
use vn30_domain::timestamp::MarketTimestamp;

pub struct EventSequencer {
    pub max_capacity: usize,
    pub allowed_lateness: Duration,
    pub max_seen_ts: Option<MarketTimestamp>,
    pub watermark: Option<MarketTimestamp>,
    pub buffers: BTreeMap<MarketTimestamp, Vec<MarketEvent>>,
    pub current_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    Buffered,
    DroppedLate {
        event_ts: MarketTimestamp,
        watermark: MarketTimestamp,
    },
}

impl EventSequencer {
    pub fn new(allowed_lateness: Duration, max_capacity: usize) -> Self {
        return Self {
            max_capacity,
            allowed_lateness,
            max_seen_ts: None,
            watermark: None,
            buffers: BTreeMap::new(),
            current_size: 0,
        };
    }
    pub fn push(&mut self, event: MarketEvent) -> PushResult {
        let event_ts = event.timestamp(); // nó sẽ trả về MarketTimestamp

        // đoạn này dùng để kiểm tra sự tồn tại của watermark có chưa
        // vì dùng option nên nế có sẵn giá trị nó sẽ trả về true và thực hiện if let some
        //trong if let some sẽ kiểm tra tiếp xem event_ts có nhỏ hơn watermark không
        // nếu nhỏ hơn thì sẽ trả về PushResult::DroppedLate
        if let Some(wt) = self.watermark {
            if event_ts < wt {
                return PushResult::DroppedLate {
                    event_ts,
                    watermark: wt,
                };
            }
        }

        // đoạn này dùng để cập nhật lại watermark
        if Some(event_ts) > self.max_seen_ts {
            self.max_seen_ts = Some(event_ts);
            let natural_wm = event_ts.checked_sub_duration(self.allowed_lateness).ok();

            self.watermark = self.watermark.max(natural_wm);
        }

        // kiểm tra khóa event_ts có tồn tại chưa
        // nếu có thì thêm vào và tăng current_size
        // nếu chưa thì khởi tạo một vector mới và thêm vào và tăng current_size
        self.buffers.entry(event_ts).or_default().push(event);
        self.current_size += 1;

        return PushResult::Buffered;
    }

    // hàm dùng để ép xả các phần tử có timestamp nhỏ hơn watermark
    // hàm này dùng để xử lý khi có sự kiện bị trễ mạng và kích thước bộ đệm vượt quá mức cho phép
    pub fn ingest(&mut self, event: MarketEvent) -> (PushResult, Vec<MarketEvent>) {
        let push_res = self.push(event);
        let mut ready = Vec::new();
        match push_res {
            PushResult::Buffered => {
                //ép xả các phần tử có timestamp nhỏ hơn watermark
                ready.append(&mut self.flush_ready());
                // ép xả khi kích thước bộ đệm vượt quá mức cho phép
                while self.current_size > self.max_capacity {
                    // nếu trong bộ đệm có sự kiện
                    if let Some(entry) = self.buffers.first_entry() {
                        // lấy sự kiện có timestamp nhỏ nhất ra
                        let (evicted_ts, mut events) = entry.remove_entry();
                        // cập nhật lại watermark
                        self.watermark = self.watermark.max(Some(evicted_ts));
                        // giảm kích thước bộ đệm
                        self.current_size -= events.len();
                        // sắp xếp lại các sự kiện theo timestamp
                        events.sort_by_key(|e| e.timestamp());
                        // thêm các sự kiện vào ready
                        ready.extend(events);
                    } else {
                        // nếu không có sự kiện nào thì thoát khỏi vòng lặp
                        break;
                    }
                }

                return (push_res, ready);
            }
            PushResult::DroppedLate { .. } => {
                return (push_res, ready);
            }
        }
    }

    pub fn flush_ready(&mut self) -> Vec<MarketEvent> {
        let mut ready_events: Vec<MarketEvent> = Vec::new();

        //đoạn này sử dụng let else nếu wm không tồn tại thì trả về ready_events
        //nó sẽ giúp giảm số dòng code và tăng tốc độ thực thi
        let Some(wm) = self.watermark else {
            return ready_events;
        };

        while let Some(entry) = self.buffers.first_entry() {
            if *entry.key() <= wm {
                let (_, events) = entry.remove_entry();
                self.current_size -= events.len();
                ready_events.extend(events);
            } else {
                break;
            }
        }

        return ready_events;
    }

    // Hàm này dùng khi hệ thống đóng kết nối (shutdown),
    //hết phiên giao dịch, hoặc khi cần xả sạch toàn bộ sự kiện còn sót lại
    //trong buffer ra ngoài (không cần chờ Watermark):
    pub fn flush_all(&mut self) -> Vec<MarketEvent> {
        let mut ready_events: Vec<MarketEvent> = Vec::new();
        let map_buffers = mem::take(&mut self.buffers);

        for (_, events) in map_buffers {
            ready_events.extend(events);
        }

        ready_events.sort_by_key(|e| e.timestamp());

        self.current_size = 0;
        return ready_events;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vn30_domain::market::Trade;

    fn mock_trade(secs: i64) -> MarketEvent {
        let ts = MarketTimestamp::from_epoch_secs(secs).unwrap();
        MarketEvent::Trade(Trade::new("HPG".to_string(), 28.5, 1000.0, ts).unwrap())
    }

    #[test]
    fn debug_flush_ready_flow() {
        println!("\n========== BẮT ĐẦU DEBUG FLUSH_READY ==========");
        // Cấu hình: trễ cho phép = 2 giây, chứa tối đa 100 phần tử
        let mut sequencer = EventSequencer::new(Duration::from_secs(2), 100);

        // 1. Nhận sự kiện lúc 10:00:01 (giây 1725530401)
        println!("\n--> Nạp sự kiện T1 (giây 1)");
        sequencer.push(mock_trade(1_725_530_401));
        println!(
            "    Watermark hiện tại: {:?}",
            sequencer.watermark.map(|w| w.timestamp_secs())
        );
        let flushed = sequencer.flush_ready();
        println!("    flush_ready() xả ra: {} sự kiện", flushed.len());
        println!(
            "    Số sự kiện còn trong buffer: {}",
            sequencer.current_size
        );

        // 2. Nhận sự kiện lúc 10:00:03 (giây 1725530403)
        println!("\n--> Nạp sự kiện T3 (giây 3)");
        sequencer.push(mock_trade(1_725_530_403));
        println!(
            "    Watermark dâng lên mốc: {:?}",
            sequencer.watermark.map(|w| w.timestamp_secs())
        );
        let flushed = sequencer.flush_ready();
        println!(
            "    flush_ready() xả ra: {} sự kiện (Timestamp: {:?})",
            flushed.len(),
            flushed
                .iter()
                .map(|e| e.timestamp().timestamp_secs())
                .collect::<Vec<_>>()
        );
        println!(
            "    Số sự kiện còn trong buffer: {}",
            sequencer.current_size
        );

        // 3. Sự kiện T2 (giây 1725530402) bị trễ mạng bay tới sau!
        println!("\n--> Nạp sự kiện T2 (giây 2 - ĐẾN SAI THỨ TỰ)");
        let res = sequencer.push(mock_trade(1_725_530_402));
        println!("    Kết quả push T2: {:?}", res);
        println!(
            "    Số sự kiện đang chờ trong buffer: {}",
            sequencer.current_size
        );

        // 4. Nhận sự kiện lúc 10:00:05 (giây 1725530405)
        println!("\n--> Nạp sự kiện T5 (giây 5)");
        sequencer.push(mock_trade(1_725_530_405));
        println!(
            "    Watermark dâng lên mốc: {:?}",
            sequencer.watermark.map(|w| w.timestamp_secs())
        );
        let flushed = sequencer.flush_ready();
        println!(
            "    flush_ready() xả ra: {} sự kiện (Theo thứ tự: {:?})",
            flushed.len(),
            flushed
                .iter()
                .map(|e| e.timestamp().timestamp_secs())
                .collect::<Vec<_>>()
        );
        println!(
            "    Số sự kiện còn lại trong buffer: {}",
            sequencer.current_size
        );
        println!("================================================\n");
    }

    #[test]
    fn test_ingest_reorders_out_of_order() {
        let mut sequencer = EventSequencer::new(Duration::from_secs(2), 100);

        let base = 1_725_530_400;
        let (res1, ready1) = sequencer.ingest(mock_trade(base + 1));
        assert_eq!(res1, PushResult::Buffered);
        assert!(ready1.is_empty());

        let (res2, ready2) = sequencer.ingest(mock_trade(base + 3));
        assert_eq!(res2, PushResult::Buffered);

        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].timestamp().timestamp_secs(), base + 1);

        let (res3, ready3) = sequencer.ingest(mock_trade(base + 2));
        assert_eq!(res3, PushResult::Buffered);
        assert!(ready3.is_empty());

        let (res4, ready4) = sequencer.ingest(mock_trade(base + 5));
        assert_eq!(res4, PushResult::Buffered);
        assert_eq!(ready4.len(), 2);
        assert_eq!(
            ready4.get(0).unwrap().timestamp().timestamp_secs(),
            base + 2
        );
        assert_eq!(
            ready4.get(1).unwrap().timestamp().timestamp_secs(),
            base + 3
        );

        let (res5, ready5) = sequencer.ingest(mock_trade(base + 10));
        assert_eq!(res5, PushResult::Buffered);
        assert_eq!(ready5.len(), 1);
        assert_eq!(
            ready5.get(0).unwrap().timestamp().timestamp_secs(),
            base + 5
        );
    }

    #[test]
    fn test_ingest_drops_late_events() {
        let mut sequencer = EventSequencer::new(Duration::from_secs(2), 100);
        let base = 1_725_530_400;

        // 1. Nạp T10 -> watermark dâng lên mốc: base + 10 - 2s = base + 8
        let (res1, ready1) = sequencer.ingest(mock_trade(base + 10));
        assert_eq!(res1, PushResult::Buffered);
        assert!(ready1.is_empty());
        assert_eq!(
            sequencer.watermark.map(|w| w.timestamp_secs()),
            Some(base + 8)
        );

        // 2. Nạp một sự kiện đến quá trễ: T7 (base + 7) < watermark (base + 8)
        let late_event = mock_trade(base + 7);
        let (res_late, ready_late) = sequencer.ingest(late_event);

        // Phải trả về DroppedLate với đúng event_ts và watermark
        assert_eq!(
            res_late,
            PushResult::DroppedLate {
                event_ts: MarketTimestamp::from_epoch_secs(base + 7).unwrap(),
                watermark: MarketTimestamp::from_epoch_secs(base + 8).unwrap(),
            }
        );
        assert!(ready_late.is_empty());

        // Sự kiện bị drop không được phép tăng dung lượng buffer (vẫn chỉ có 1 phần tử T10)
        assert_eq!(sequencer.current_size, 1);
    }

    #[test]
    fn test_ingest_capacity_overflow_forces_eviction() {
        // Cấu hình: allowed_lateness = 10s (độ trễ lớn), max_capacity = 2 (chỉ chứa tối đa 2 sự kiện)
        let mut sequencer = EventSequencer::new(Duration::from_secs(10), 2);
        let base = 1_725_530_400;

        // 1. Nạp T10 (giây 10) -> buffer chứa [T10] (size = 1)
        let (res1, ready1) = sequencer.ingest(mock_trade(base + 10));
        assert_eq!(res1, PushResult::Buffered);
        assert!(ready1.is_empty());
        assert_eq!(sequencer.current_size, 1);

        // 2. Nạp T12 (giây 12) -> buffer chứa [T10, T12] (size = 2, đầy capacity)
        let (res2, ready2) = sequencer.ingest(mock_trade(base + 12));
        assert_eq!(res2, PushResult::Buffered);
        assert!(ready2.is_empty());
        assert_eq!(sequencer.current_size, 2);

        // 3. Nạp T14 (giây 14) -> nếu không xả thì size = 3 > max_capacity (2)
        // Hệ thống phải ép xả T10 ra ngoài và nâng watermark lên bằng T10!
        let (res3, ready3) = sequencer.ingest(mock_trade(base + 14));
        assert_eq!(res3, PushResult::Buffered);

        // Kiểm tra T10 đã bị ép xả ra
        assert_eq!(ready3.len(), 1);
        assert_eq!(
            ready3.get(0).unwrap().timestamp().timestamp_secs(),
            base + 10
        );

        // Dung lượng buffer được giới hạn cứng, không vượt quá max_capacity = 2
        assert_eq!(sequencer.current_size, 2);

        // Watermark phải được nâng lên mốc T10 để bảo vệ downstream
        assert!(sequencer.watermark.unwrap().timestamp_secs() >= base + 10);
    }

    #[test]
    fn test_flush_all() {
        let mut sequencer = EventSequencer::new(Duration::from_secs(10), 100);
        let base = 1_725_530_400;

        // Nạp các sự kiện đến lộn xộn: T2, T4, T1
        sequencer.ingest(mock_trade(base + 2));
        sequencer.ingest(mock_trade(base + 4));
        sequencer.ingest(mock_trade(base + 1));

        assert_eq!(sequencer.current_size, 3);

        // Gọi flush_all để dọn sạch buffer khi đóng phiên
        let flushed = sequencer.flush_all();

        // Phải xả đủ 3 sự kiện và buffer cạn về 0
        assert_eq!(flushed.len(), 3);
        assert_eq!(sequencer.current_size, 0);
        assert!(sequencer.buffers.is_empty());

        // Các sự kiện xả ra phải được sắp xếp theo đúng thứ tự thời gian tăng dần: T1 -> T2 -> T4
        assert_eq!(flushed[0].timestamp().timestamp_secs(), base + 1);
        assert_eq!(flushed[1].timestamp().timestamp_secs(), base + 2);
        assert_eq!(flushed[2].timestamp().timestamp_secs(), base + 4);
    }
}
