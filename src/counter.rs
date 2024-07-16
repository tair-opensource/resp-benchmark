use std::cmp::min;
use std::sync::atomic::AtomicU64;
use tokio::sync::Notify;

pub(crate) struct Counter {
    current_count: AtomicU64,
    max_count: u64,
    stop_notify: Notify,
}
impl Counter {
    pub fn new(max_count: u64) -> Self {
        Counter {
            current_count: AtomicU64::new(0),
            max_count,
            stop_notify: Notify::new(),
        }
    }
    pub fn stop(&self) {
        self.stop_notify.notify_waiters();
    }
    pub async fn wait_stop(&self) {
        self.stop_notify.notified().await;
    }
    pub fn fetch(&self, count: u64) -> u64 {
        let prev_count = self.current_count.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
        if prev_count >= self.max_count {
            return 0;
        }
        return min(self.max_count - prev_count, count);
    }
}
