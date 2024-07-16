use crate::client::ClientConfig;
use crate::command::Command;
use clap::{command, Parser};
use number_range::NumberRangeOptions;
use serde::{Deserialize, Deserializer};
use std::process::exit;

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub command: Command,
    pub count: u64,
}

impl<'de> Deserialize<'de> for Command {
    fn deserialize<D>(deserializer: D) -> Result<Command, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Command::new(s.as_str()))
    }
}
const fn _default_1() -> u64 {
    1
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: Option<String>,
    pub dataset: Option<Dataset>,
    pub command: Command,
    pub connections: u64,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub seconds: u64,
    #[serde(default = "_default_1")]
    pub pipeline: u64,
}

impl Default for Case {
    fn default() -> Self {
        Case {
            name: None,
            dataset: None,
            command: Command::new("PING"),
            connections: 0,
            count: 0,
            seconds: 0,
            pipeline: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    #[serde(rename = "xlsx")]
    XLSX,

    #[serde(rename = "json")]
    JSON,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(flatten)]
    pub client_config: ClientConfig,
    #[serde(default)]
    pub cpus: CPUS,
    #[serde(default)]
    #[serde(rename = "output")]
    pub output_formats: Vec<OutputFormat>,
    #[serde(default)]
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub struct CPUS(Vec<u64>);

impl Default for CPUS {
    fn default() -> Self {
        let vec: Vec<u64> = (0..num_cpus::get() as u64).collect();
        CPUS(vec)
    }
}

impl CPUS {
    pub fn iter(&self) -> std::slice::Iter<u64> {
        self.0.iter()
    }
    pub fn get(&self, idx: usize) -> u64 {
        self.0[idx]
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'de> Deserialize<'de> for CPUS {
    fn deserialize<D>(deserializer: D) -> Result<CPUS, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            let vec: Vec<u64> = (0..num_cpus::get() as u64).collect();
            Ok(CPUS(vec))
        } else {
            let vec: Vec<u64> = NumberRangeOptions::new().with_list_sep(',').with_range_sep('-').parse(s.as_str()).unwrap().collect();
            Ok(CPUS(vec))
        }
    }
}

const AFTER_HELP: &str = r#""#;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// Path to the workload file, e.g., `./workload/redis.toml`
    workload: String,
}

impl Config {
    pub fn parse() -> Config {
        let cli = Cli::parse();
        return Config::from_file(&cli.workload);
    }

    pub fn from_file(path: &str) -> Config {
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Could not read file `{}`: {}", path, e);
                exit(1);
            }
        };

        let conf: Config = match toml::from_str(&contents) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Could not parse file `{}`: {}", path, e);
                exit(1);
            }
        };
        return conf;
    }

    pub fn client_config(&self) -> ClientConfig {
        self.client_config.clone()
    }
}

#[cfg(test)]
mod tests {
    use number_range::NumberRangeOptions;

    #[test]
    fn test() {
        let v: Vec<usize> = NumberRangeOptions::new().with_list_sep(',').with_range_sep('-').parse("1,3-10,14").unwrap().collect();
        println!("{:?}", v);
    }
}
