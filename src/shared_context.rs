use crate::async_flag::AsyncFlag;
use crate::histogram::Histogram;
use std::cmp::min;
use std::option::Option;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Clone)]
pub struct SharedContext {
    pub is_loading: bool,
    // limit by max_count
    database_counts: Arc<Vec<AtomicU64>>,
    next_database: Arc<AtomicUsize>,
    max_count: u64,

    // limit by max_seconds
    instant: Arc<RwLock<Option<Instant>>>,
    max_seconds: u64,

    // stop flag
    stop_flag: AsyncFlag,

    // histogram
    pub histogram: Arc<Histogram>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divides_requests_evenly_with_partial_pipelines() {
        let context = SharedContext::new(100, 0, true, 3);
        let mut totals = [0; 3];
        while let Some(database) = context.next_database() {
            loop {
                let count = context.fetch(database, 10);
                if count == 0 {
                    break;
                }
                totals[database] += count;
            }
        }
        assert_eq!(totals, [34, 33, 33]);
    }

    #[test]
    fn concurrent_workers_cannot_exceed_database_quotas() {
        let context = SharedContext::new(10003, 0, false, 4);
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let context = context.clone();
                std::thread::spawn(move || {
                    let mut totals = [0; 4];
                    while let Some(database) = context.next_database() {
                        totals[database] += context.fetch(database, 17);
                        std::thread::yield_now();
                    }
                    totals
                })
            })
            .collect();
        let mut totals = [0; 4];
        for worker in workers {
            for (database, count) in worker.join().unwrap().iter().enumerate() {
                totals[database] += count;
            }
        }
        assert_eq!(totals, [2501, 2501, 2501, 2500]);
    }

    #[test]
    fn skips_zero_quotas_when_requests_are_fewer_than_databases() {
        let context = SharedContext::new(2, 0, false, 4);
        assert_eq!(context.next_database(), Some(0));
        assert_eq!(context.fetch(0, 10), 1);
        assert_eq!(context.next_database(), Some(1));
        assert_eq!(context.fetch(1, 10), 1);
        assert_eq!(context.next_database(), None);
    }

    #[test]
    fn unlimited_requests_respect_stop_and_time_limits() {
        let mut context = SharedContext::new(0, 0, false, 2);
        for database in [0, 1, 0, 1] {
            assert_eq!(context.next_database(), Some(database));
            assert_eq!(context.fetch(database, 10), 10);
        }
        context.stop();
        assert_eq!(context.next_database(), None);
        assert_eq!(context.fetch(0, 10), 0);

        let context = SharedContext::new(0, 1, false, 2);
        *context.instant.write().unwrap() = Some(Instant::now() - std::time::Duration::from_secs(2));
        assert_eq!(context.next_database(), None);
        assert_eq!(context.fetch(0, 10), 0);
    }
}

impl SharedContext {
    pub fn new(max_count: u64, max_seconds: u64, is_loading: bool, database_count: usize) -> Self {
        assert!(database_count > 0);
        SharedContext {
            is_loading,
            database_counts: Arc::new((0..database_count).map(|_| AtomicU64::new(0)).collect()),
            next_database: Arc::new(AtomicUsize::new(0)),
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

    fn stopped(&self) -> bool {
        if self.max_seconds != 0 && self.instant.read().unwrap().is_some() {
            let elapsed = self.instant.read().unwrap().unwrap().elapsed().as_secs();
            if elapsed >= self.max_seconds {
                return true;
            }
        }

        self.stop_flag.flag()
    }

    fn database_quota(&self, database: usize) -> u64 {
        let databases = self.database_counts.len() as u64;
        self.max_count / databases + u64::from((database as u64) < self.max_count % databases)
    }

    pub fn next_database(&self) -> Option<usize> {
        if self.stopped() {
            return None;
        }
        let start = self.next_database.fetch_add(1, Ordering::Relaxed) % self.database_counts.len();
        for offset in 0..self.database_counts.len() {
            let database = (start + offset) % self.database_counts.len();
            if self.max_count == 0 || self.database_counts[database].load(Ordering::Relaxed) < self.database_quota(database) {
                return Some(database);
            }
        }
        None
    }

    pub fn fetch(&self, database: usize, count: u64) -> u64 {
        if self.stopped() {
            return 0;
        }
        if self.max_count == 0 {
            return count;
        }
        let quota = self.database_quota(database);
        // Reserve only the remaining quota, including a partial final pipeline.
        match self.database_counts[database].fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| if current < quota { Some(current + min(quota - current, count)) } else { None }) {
            Ok(previous) => min(quota - previous, count),
            Err(_) => 0,
        }
    }
}
