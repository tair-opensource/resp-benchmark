use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::Notify;

const MAX_CONN: u64 = if cfg!(target_os = "macos") { 64 } else { 1024 }; // 1024 is enough for most cases

pub struct Limiter {
    pub tcp_conn: u64,
    active_conn: AtomicU64,
    target_conn: AtomicU64,

    notify_add: Notify,
}

impl Limiter {
    pub fn new(tcp_conn: u64) -> Self {
        Limiter {
            tcp_conn,
            active_conn: AtomicU64::new(0),
            target_conn: AtomicU64::new(0),
            notify_add: Notify::new(),
        }
    }
    pub async fn wait_add(&self) {
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
        let old_val = self.target_conn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if old_val >= self.tcp_conn {
            return;
        }
        loop {
            self.notify_add.notify_waiters();
            let active_conn = self.active_conn.load(std::sync::atomic::Ordering::SeqCst);
            let target_conn = self.target_conn.load(std::sync::atomic::Ordering::SeqCst);
            if active_conn >= target_conn {
                break;
            }
        }
    }
    pub fn get_active_conn(&self) -> u64 {
        self.active_conn.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn get_target_conn(&self) -> u64 {
        self.target_conn.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub struct AutoLimiter {
    pub auto: bool,
    pub ready: bool,
    pub limiters: Vec<Arc<Limiter>>,
    max_conn: u64,
    target_conn: u64,

    last_cnt: u64,
    last_qps: f64,
    last_last_qps: f64,
    instant: std::time::Instant,
    step: u64,
    inx: usize,
}

impl AutoLimiter {
    pub fn new(mut max_conn: u64, count: u64) -> Self {
        let mut limiters = Vec::new();
        let auto = max_conn == 0;
        if auto {
            max_conn = MAX_CONN;
        }
        let mut total_connection = max_conn;
        let mut left_count = count;
        for _ in 0..count {
            let my_conn = (total_connection + left_count - 1) / left_count;
            limiters.push(Arc::new(Limiter::new(my_conn)));
            total_connection -= my_conn;
            left_count -= 1;
        }
        AutoLimiter {
            auto,
            ready: false,
            limiters,
            max_conn,
            target_conn: 0,

            last_cnt: 0,
            last_qps: 0.0,
            last_last_qps: 0.0,
            instant: std::time::Instant::now(),
            step: 1,
            inx: 0,
        }
    }

    pub fn active_conn(&self) -> u64 {
        self.limiters.iter().map(|limiter| limiter.get_active_conn()).sum()
    }
    pub fn target_conn(&self) -> u64 {
        self.limiters.iter().map(|limiter| limiter.get_target_conn()).sum()
    }

    pub fn adjust(&mut self, cnt: u64) -> bool {
        if self.ready {
            return false;
        }
        if !self.auto {
            for _ in 0..self.max_conn {
                self.limiters[self.inx].add_conn();
                self.inx = (self.inx + 1) % self.limiters.len();
                self.target_conn += 1;
            }
            self.ready = true;
            return false;
        }

        let target_conn: u64 = self.limiters.iter().map(|limiter| limiter.get_target_conn()).sum();
        let qps = (cnt - self.last_cnt) as f64 / self.instant.elapsed().as_secs_f64();

        if qps >= self.last_qps * 1.1 && target_conn + self.step * 2 < MAX_CONN {
            self.step = self.step * 2;
            self.last_last_qps = self.last_qps;
            self.last_qps = qps;
        } else {
            self.step = 0;
            self.ready = true;
            return false;
        }
        for _ in 0..self.step {
            self.limiters[self.inx].add_conn();
            self.inx = (self.inx + 1) % self.limiters.len();
        }
        self.last_cnt = cnt;
        self.instant = std::time::Instant::now();
        return true;
    }
}
