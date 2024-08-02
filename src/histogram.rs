use std::fmt::Display;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Histogram {
    cnt: AtomicU64,
    buckets: Vec<AtomicU64>,
}

impl Histogram {
    // PRECISION:
    // 0-99: 10us <1ms
    // 100-199: 100us <10ms
    // 200-299: 1ms <100ms
    // 300-399: 10ms <1s
    // 400-499: 100ms <10s
    // 500: >=10s
    pub fn new() -> Histogram {
        let mut buckets = Vec::with_capacity(501);
        for _ in 0..501 {
            buckets.push(AtomicU64::new(0));
        }
        Histogram {
            cnt: AtomicU64::new(0),
            buckets,
        }
    }

    pub fn record(&self, latency_us: u64) {
        let index = match latency_us {
            0..=999 => latency_us / 10,                            // <1ms precision 10us
            1_000..=9_999 => 100 + (latency_us / 100),             // <10ms precision 100us
            10_000..=99_999 => 200 + (latency_us / 1_000),         // <100ms precision 1ms
            100_000..=999_999 => 300 + (latency_us / 10_000),      // <1s precision 10ms
            1_000_000..=9_999_999 => 400 + (latency_us / 100_000), // <10s precision 100ms
            _ => 500,                                              // >=10s
        };
        self.cnt.fetch_add(1, Ordering::Relaxed);
        self.buckets[index as usize].fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.cnt.store(0, Ordering::Relaxed);
        for i in 0..self.buckets.len() {
            self.buckets[i].store(0, Ordering::Relaxed);
        }
    }

    #[allow(dead_code)]
    pub fn un_record(&self, latency_us: u64) {
        let index = match latency_us {
            0..=999 => latency_us / 10,                            // <1ms precision 10us
            1_000..=9_999 => 100 + (latency_us / 100),             // <10ms precision 100us
            10_000..=99_999 => 200 + (latency_us / 1_000),         // <100ms precision 1ms
            100_000..=999_999 => 300 + (latency_us / 10_000),      // <1s precision 10ms
            1_000_000..=9_999_999 => 400 + (latency_us / 100_000), // <10s precision 100ms
            _ => 500,                                              // >=10s
        };
        self.cnt.fetch_sub(1, Ordering::Relaxed);
        self.buckets[index as usize].fetch_sub(1, Ordering::Relaxed);
    }

    fn bucket_unit_us(index: u64) -> u64 {
        match index {
            0..=99 => index * 10,                 // precision 10us
            100..=199 => (index - 100) * 100,     // precision 100us
            200..=299 => (index - 200) * 1_000,   // precision 1ms
            300..=399 => (index - 300) * 10_000,  // precision 10ms
            400..=499 => (index - 400) * 100_000, // precision 100ms
            _ => 10_000_000,                      // >=10s
        }
    }

    pub fn cnt(&self) -> u64 {
        self.cnt.load(Ordering::Relaxed)
    }

    pub fn avg(&self) -> u64 {
        let cnt = self.cnt();
        if cnt == 0 {
            return 0;
        }
        let mut sum = 0;
        for i in 0..self.buckets.len() {
            sum += Histogram::bucket_unit_us(i as u64) * self.buckets[i].load(Ordering::Relaxed);
        }
        sum / cnt
    }

    pub fn percentile(&self, percentile: f64) -> u64 {
        let cnt = self.cnt();
        if cnt == 0 {
            return 0;
        }
        let mut sum = 0;
        let target = (cnt as f64 * percentile) as u64;
        for i in 0..self.buckets.len() {
            sum += self.buckets[i].load(Ordering::Relaxed);
            if sum > 0 && sum >= target {
                return Histogram::bucket_unit_us(i as u64);
            }
        }
        0
    }

    fn humanize_us(latency_us: u64) -> String {
        match latency_us {
            0 => "<0.01ms".to_string(),
            1..=999 => format!("{:.2}ms", latency_us as f64 / 1_000.0),
            1_000..=9_999 => format!("{:.1}ms", latency_us as f64 / 1_000.0),
            10_000..=99_999 => format!("{:.0}ms", latency_us as f64 / 1_000.0),
            100_000..=999_999 => format!("{:.2}s", latency_us as f64 / 1_000_000.0),
            1_000_000..=9_999_999 => format!("{:.1}s", latency_us as f64 / 1_000_000.0),
            _ => ">10s".to_string(),
        }
    }
}

impl Display for Histogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cnt = self.cnt();
        if cnt == 0 {
            return write!(f, "no data");
        }
        let avg = self.avg();
        let p99 = self.percentile(0.99);

        write!(f, "cnt: {}, avg: {}, p99: {}", cnt, Histogram::humanize_us(avg), Histogram::humanize_us(p99))
    }
}

#[cfg(test)]
mod tests {
    use super::Histogram;

    #[test]
    fn test() {
        let histogram = Histogram::new();
        for i in 0..1000 {
            histogram.record(i * 1000);
        }
        println!("{}", histogram);

        // PRECISION:
        // 0-99: 10us <1ms
        // 100-199: 100us <10ms
        // 200-299: 1ms <100ms
        // 300-399: 10ms <1s
        // 400-499: 100ms <10s
        // 500: >=10s
        let src = [0, 9, 10, 99, 100, 999, 1_000, 9_999, 10_000, 99_999, 100_000, 999_999, 1_000_000, 9_999_999, 10_000_000, 99_999_999, 100_000_000];
        let dst = [0, 0, 10, 90, 100, 990, 1_000, 9_900, 10_000, 99_000, 100_000, 990_000, 1_000_000, 9_900_000, 10_000_000, 10_000_000, 10_000_000];
        for i in 0..src.len() {
            let histogram = Histogram::new();
            for _ in 0..1000 {
                histogram.record(src[i]);
            }

            println!("src: {} dst: {}", src[i], dst[i]);
            println!("{}", histogram);
            assert_eq!(histogram.cnt(), 1000);
            assert_eq!(histogram.avg(), dst[i]);
            assert_eq!(histogram.percentile(0.0), dst[i]);
            assert_eq!(histogram.percentile(0.5), dst[i]);
            assert_eq!(histogram.percentile(0.99), dst[i]);
            assert_eq!(histogram.percentile(0.999), dst[i]);
            assert_eq!(histogram.percentile(1.0), dst[i]);
        }
    }
}
