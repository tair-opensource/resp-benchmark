use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use rand::distributions::Distribution;

#[derive(Clone, Debug)]
pub enum DistributionEnum {
    Uniform(rand::distributions::Uniform<u64>),
    Zipfian(zipf::ZipfDistribution),
    Sequence(SequenceDistribution),
}

impl DistributionEnum {
    pub fn new(s: &str, range: u64) -> Self {
        match s {
            "uniform" => Self::Uniform(rand::distributions::Uniform::new(0, range)),
            "zipfian" => Self::Zipfian(zipf::ZipfDistribution::new(range as usize, 1.03).unwrap()),
            "sequence" => Self::Sequence(SequenceDistribution::new(range)),
            _ => panic!("Unknown distribution"),
        }
    }
    pub fn sample(&mut self, rng: &mut impl rand::Rng) -> u64 {
        match self {
            Self::Uniform(d) => d.sample(rng),
            Self::Zipfian(d) => d.sample(rng) as u64,
            Self::Sequence(d) => d.sample(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SequenceDistribution {
    range: u64,
    current: Arc<AtomicU64>,
}

impl SequenceDistribution {
    fn new(range: u64) -> Self {
        Self { range, current: Arc::new(AtomicU64::new(0)) }
    }
    fn sample(&mut self) -> u64 {
        let ret = self.current.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ret % self.range
    }
}