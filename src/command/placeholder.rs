use std::cmp::min;
use std::process::exit;
use crate::command::distribution::DistributionEnum;
use std::str::FromStr;
use rand::{distributions::Alphanumeric, thread_rng, Rng};

#[derive(Debug, Clone)]
pub enum PlaceholderEnum {
    String(PlaceholderString),
    Key(PlaceholderKey),
    Value(PlaceholderValue),
    Rand(PlaceholderRand),
    Range(PlaceholderRange),
}

impl PlaceholderEnum {
    pub fn new_string(str: &str) -> Self {
        Self::String(PlaceholderString::new(str.to_string()))
    }
    pub fn new(str: &str) -> Self {
        let s = str.to_string();
        let words: Vec<&str> = s.split_whitespace().collect();
        if words.len() == 0 {
            eprint!("placeholder is empty");
            exit(1);
        }
        let ph = match words[0] {
            "key" => {
                if words.len() != 3 {
                    eprint!("wrong number of arguments for key placeholder: {:?}", words);
                    exit(1);
                }
                let range = u64::from_str(words[2]).unwrap();
                let distribution = DistributionEnum::new(words[1], range);
                PlaceholderEnum::Key(PlaceholderKey::new(distribution))
            }
            "value" => {
                if words.len() != 2 {
                    eprint!("wrong number of arguments for value placeholder: {:?}", words);
                    exit(1);
                }
                let size = u64::from_str(words[1]).unwrap();
                PlaceholderEnum::Value(PlaceholderValue::new(size))
            }
            "rand" => {
                if words.len() != 2 {
                    eprint!("wrong number of arguments for rand placeholder: {:?}", words);
                    exit(1);
                }
                PlaceholderEnum::Rand(PlaceholderRand::new(u64::from_str(words[1]).unwrap()))
            }
            "range" => {
                if words.len() != 3 {
                    eprint!("wrong number of arguments for range placeholder: {:?}", words);
                    exit(1);
                }
                let range = u64::from_str(words[1]).unwrap();
                let width = u64::from_str(words[2]).unwrap();
                PlaceholderEnum::Range(PlaceholderRange::new(range, width))
            }
            name => {
                eprint!("Invalid placeholder: {}", name);
                exit(1);
            }
        };
        ph
    }
    pub fn gen(&mut self) -> Vec<String> {
        match self {
            Self::String(p) => vec![p.gen()],
            Self::Key(p) => vec![p.gen()],
            Self::Value(p) => vec![p.gen()],
            Self::Rand(p) => vec![p.gen()],
            Self::Range(p) => p.gen(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderString {
    value: String,
}

impl PlaceholderString {
    pub fn new(value: String) -> Self {
        Self { value }
    }
    fn gen(&mut self) -> String {
        self.value.clone()
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderKey {
    distribution: DistributionEnum,
}

impl PlaceholderKey {
    fn new(distribution: DistributionEnum) -> Self {
        Self { distribution }
    }
    fn gen(&mut self) -> String {
        format!("key_{:010}", self.distribution.sample(&mut rand::thread_rng()))
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderValue {
    size: usize,
}

impl PlaceholderValue {
    pub fn new(size: u64) -> Self {
        Self { size: size as usize }
    }
    pub fn gen(&self) -> String {
        let rng = thread_rng();
        let chars: String = rng.sample_iter(&Alphanumeric).take(self.size).map(char::from).collect();
        chars
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderRand {
    distribution: DistributionEnum,
}

impl PlaceholderRand {
    pub fn new(range: u64) -> Self {
        Self { distribution: DistributionEnum::new("uniform", range) }
    }
    fn gen(&mut self) -> String {
        format!("{}", self.distribution.sample(&mut thread_rng()))
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderRange {
    distribution: DistributionEnum,
    range: u64,
    width: u64,
}

impl PlaceholderRange {
    pub fn new(range: u64, width: u64) -> Self {
        Self {
            distribution: DistributionEnum::new("uniform", range),
            range,
            width,
        }
    }
    fn gen(&mut self) -> Vec<String> {
        let left = self.distribution.sample(&mut thread_rng());
        let right = min(left + self.width, self.range - 1);
        vec![left.to_string(), right.to_string()]
    }
}

