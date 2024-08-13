use crate::async_flag::AsyncFlag;
use crate::histogram::Histogram;
use std::cmp::min;
use std::option::Option;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Clone)]
pub struct SharedContext {
    pub is_loading: bool,
    // limit by max_count
    current_count: Arc<AtomicU64>,
    max_count: u64,

    // limit by max_seconds
    instant: Arc<RwLock<Option<Instant>>>,
    max_seconds: u64,

    // stop flag
    stop_flag: AsyncFlag,

    // histogram
    pub histogram: Arc<Histogram>,
}

impl SharedContext {
    pub fn new(max_count: u64, max_seconds: u64, is_loading: bool) -> Self {
        SharedContext {
            is_loading,
            current_count: Arc::new(AtomicU64::new(0)),
            max_count,
            instant: Arc::new(RwLock::new(None)),
            max_seconds,
            stop_flag: AsyncFlag::new(),

            histogram: Arc::new(Histogram::new()),
        }
    }

    pub fn stop(&mut self) {
        self.stop_flag.set_flag();
    }

    pub async fn wait_stop(&mut self) {
        self.stop_flag.wait_flag().await;
    }

    pub fn start_timer(&mut self) {
        let mut instant = self.instant.write().unwrap();
        *instant = Some(Instant::now());
    }

    pub fn fetch(&self, count: u64) -> u64 {
        let mut result = count;
        if self.max_count != 0 {
            let prev_count = self.current_count.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
            if prev_count >= self.max_count {
                return 0;
            }
            result = min(self.max_count - prev_count, count);
        }

        if self.max_seconds != 0 && self.instant.read().unwrap().is_some() {
            let elapsed = self.instant.read().unwrap().unwrap().elapsed().as_secs();
            if elapsed >= self.max_seconds {
                return 0;
            }
        }
        return result;
    }
}
