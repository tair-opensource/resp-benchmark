use redis;

mod distribution;
mod lua;
mod parser;
mod placeholder;

pub use placeholder::PlaceholderGenerator;
pub use lua::LuaGenerator;

pub trait CommandGenerator: Send + std::string::ToString {
    fn gen_cmd(&mut self) -> redis::Cmd;
    fn gen_cmd_with_lock(&mut self) -> redis::Cmd;
    fn clone_box(&self) -> Box<dyn CommandGenerator>;
}