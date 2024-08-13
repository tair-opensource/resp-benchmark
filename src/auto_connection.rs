use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::Notify;
use crate::histogram::Histogram;

const MAX_CONN: u64 = if cfg!(target_os = "macos") { 64 } else { 1024 }; // 1024 is enough for most cases

pub struct ConnLimiter {
    pub total_conn: u64, // total connection count >= active_conn
    active_conn: AtomicU64,
    target_conn: AtomicU64, // The target connection count

    notify_add: Notify,
}

impl ConnLimiter {
    pub fn new(total_conn: u64, target_conn: u64) -> Self {
        ConnLimiter {
            total_conn,
            active_conn: AtomicU64::new(0),
            target_conn: AtomicU64::new(target_conn),
            notify_add: Notify::new(),
        }
    }
    pub async fn wait_new_conn(&self) {
        let active_conn = self.active_conn.load(std::sync::atomic::Ordering::SeqCst);
        let target_conn = self.target_conn.load(std::sync::atomic::Ordering::SeqCst);
        if active_conn < target_conn {
            self.active_conn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return;
        }
        loop {
            self.notify_add.notified().await;
            let active_conn = self.active_conn.load(std::sync::atomic::Ordering::SeqCst);
            let target_conn = self.target_conn.load(std::sync::atomic::Ordering::SeqCst);
            if active_conn >= target_conn {
                continue;
            }
            let old_value = self.active_conn.compare_exchange(active_conn, active_conn + 1, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).unwrap();
            if old_value != active_conn {
                continue;
            }
            break;
        }
    }
    pub fn add_conn(&self) {
        let target_conn = self.target_conn.load(std::sync::atomic::Ordering::SeqCst);
        if target_conn >= self.total_conn {
            return;
        }
        self.target_conn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        loop {
            self.notify_add.notify_one();
            let active_conn = self.active_conn.load(std::sync::atomic::Ordering::SeqCst);
            let target_conn = self.target_conn.load(std::sync::atomic::Ordering::SeqCst);
            if active_conn >= target_conn {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    pub fn get_active_conn(&self) -> u64 {
        self.active_conn.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_target_conn(&self) -> u64 {
        self.target_conn.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub struct AutoConnection {
    pub ready: bool,
    pub limiters: Vec<Arc<ConnLimiter>>,

    last_cnt: u64,
    last_qps: f64,
    instant: std::time::Instant,
    inx: usize,
}

impl AutoConnection {
    pub fn new(active_conn: u64, thread_count: u64) -> Self {
        let mut limiters = Vec::new();
        let auto = active_conn == 0;
        let mut total_connection = if auto { MAX_CONN } else { active_conn };
        let mut left_count = thread_count;
        for _ in 0..thread_count {
            let my_conn = (total_connection + left_count - 1) / left_count;
            let target_conn = if auto { 0 } else { my_conn };
            let conn_limiter = Arc::new(ConnLimiter::new(my_conn, target_conn));
            limiters.push(conn_limiter);
            total_connection -= my_conn;
            left_count -= 1;
        }
        AutoConnection {
            ready: !auto,
            limiters,

            last_cnt: 0,
            last_qps: 0.0,
            instant: std::time::Instant::now(),
            inx: 0,
        }
    }

    pub fn active_conn(&self) -> u64 {
        self.limiters.iter().map(|limiter| limiter.get_active_conn()).sum()
    }
    #[allow(dead_code)]
    pub fn target_conn(&self) -> u64 {
        self.limiters.iter().map(|limiter| limiter.get_target_conn()).sum()
    }

    pub fn adjust(&mut self, h: &Histogram) {
        if self.ready {
            return;
        }

        let elapsed = self.instant.elapsed().as_secs_f64();
        if elapsed < 0.5 {
            return;
        }
        let qps = (h.cnt() - self.last_cnt) as f64 / elapsed;
        let need_add_conn;
        if qps >= self.last_qps * 2.0 || elapsed >= 3f64 {
            if self.last_qps == 0.0 {
                need_add_conn = 1; // at least 1 connection
            } else if qps > self.last_qps * 1.3 {
                need_add_conn = self.active_conn();
            } else {
                self.ready = true;
                return;
            }
        } else {
            return;
        }
        for _ in 0..need_add_conn {
            self.limiters[self.inx].add_conn();
            self.inx = (self.inx + 1) % self.limiters.len();
        }
        self.last_qps = qps;
        self.last_cnt = h.cnt();
        self.instant = std::time::Instant::now();
        return;
    }
}
