use crate::command::placeholder::PlaceholderEnum;
use redis;
use std::sync::{Arc, Mutex};

mod distribution;
mod parser;
mod placeholder;

#[derive(Clone, Debug)]
pub struct Command {
    str: String,
    argv: Vec<PlaceholderEnum>,
    #[allow(dead_code)]
    lock: Arc<Mutex<()>>,
}

impl Command {
    pub fn new(cmd: &str) -> Command {
        let prev_cmd = cmd;
        match parser::parse_all(cmd) {
            Ok((nm, args)) => {
                assert_eq!(nm, "");
                Command {
                    str: prev_cmd.to_string(),
                    argv: args,
                    lock: Arc::new(Mutex::new(())),
                }
            }
            Err(e) => {
                panic!("cmd parse error. cmd: {}, error: {:?}", cmd, e);
            }
        }
    }
    pub fn gen_cmd(&mut self) -> redis::Cmd {
        let mut cmd = redis::Cmd::new();
        let mut cmd_str = String::new();
        for ph in self.argv.iter_mut() {
            for arg in ph.gen() {
                cmd_str.push_str(&arg);
            }
        }
        for word in cmd_str.split_whitespace() {
            cmd.arg(word);
        }
        cmd
    }
    #[allow(dead_code)]
    pub fn gen_cmd_with_lock(&mut self) -> redis::Cmd {
        let _lock = self.lock.lock().unwrap();
        let mut cmd = redis::Cmd::new();
        let mut cmd_str = String::new();
        for ph in self.argv.iter_mut() {
            for arg in ph.gen() {
                cmd_str.push_str(&arg);
            }
        }
        for word in cmd_str.split_whitespace() {
            cmd.arg(word);
        }
        cmd
    }
    pub fn to_string(&self) -> String {
        self.str.clone()
    }
}
