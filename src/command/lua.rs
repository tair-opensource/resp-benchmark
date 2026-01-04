use crate::command::CommandGenerator;
use crate::command::distribution::DistributionEnum;
use mlua::MetaMethod;
use mlua::prelude::*;
use rand::Rng;
use rand::distributions::Alphanumeric;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// KeyGenerator provides distribution-based key generation for Lua scripts.
#[derive(Clone)]
struct KeyGenerator {
    distribution: DistributionEnum,
}

impl KeyGenerator {
    /// Creates a new KeyGenerator with the specified distribution and range.
    fn new(distribution_type: &str, range: u64) -> Self {
        let distribution = DistributionEnum::new(distribution_type, range);
        KeyGenerator { distribution }
    }

    /// Generates the next key value based on the configured distribution.
    fn generate(&mut self) -> String {
        let mut rng = rand::thread_rng();
        let value = self.distribution.sample(&mut rng);
        value.to_string()
    }
}

// Implement LuaUserData trait to make KeyGenerator usable as Lua userdata
impl LuaUserData for KeyGenerator {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method_mut(MetaMethod::Call, |_, this, _: ()| Ok(this.generate()));
    }
}

/// ValueGenerator provides random string generation with specified length for Lua scripts.
struct ValueGenerator {
    size: usize,
}

impl ValueGenerator {
    /// Creates a new ValueGenerator with the specified string length.
    fn new(size: u64) -> Self {
        ValueGenerator { size: size as usize }
    }

    /// Generates a random string of the specified length.
    fn generate(&self) -> String {
        let rng = rand::thread_rng();
        let chars: String = rng.sample_iter(Alphanumeric).take(self.size).map(char::from).collect();
        chars
    }
}

// Implement LuaUserData trait to make ValueGenerator usable as Lua userdata
impl LuaUserData for ValueGenerator {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Call, |_, this, _: ()| Ok(this.generate()));
    }
}

/// RandGenerator provides random integer generation within a specified range for Lua scripts.
struct RandGenerator {
    range: u64,
}

impl RandGenerator {
    /// Creates a new RandGenerator with the specified range.
    fn new(range: u64) -> Self {
        RandGenerator { range }
    }

    /// Generates a random u64 integer in the range [0, range).
    fn generate(&self) -> u64 {
        if self.range == 0 {
            return 0;
        }
        let mut rng = rand::thread_rng();
        rng.gen_range(0..self.range)
    }
}

// Implement LuaUserData trait to make RandGenerator usable as Lua userdata
impl LuaUserData for RandGenerator {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Call, |_, this, _: ()| Ok(this.generate()));
    }
}

type SharedKeyGenerator = Arc<Mutex<HashMap<String, KeyGenerator>>>;

#[allow(dead_code)]
pub struct LuaGenerator {
    script: String,
    lua: Lua,
    cmd: LuaFunction,
    // Shared mapping table for named KeyGenerators across cloned LuaGenerator instances
    // to allow all sequence generators grow together
    key_generators: SharedKeyGenerator,
}

impl LuaGenerator {
    pub fn new(script: &str) -> LuaGenerator {
        let key_generators = Arc::new(Mutex::new(HashMap::new()));
        let (lua, cmd) = Self::setup_lua_vm(script, key_generators.clone());
        let script = script.to_string();
        LuaGenerator { script, lua, cmd, key_generators }
    }

    /// Sets up a new Lua virtual machine with the bench object and shared key generators.
    fn setup_lua_vm(script: &str, key_generators: SharedKeyGenerator) -> (Lua, LuaFunction) {
        let lua = Lua::new();

        Self::inject_bench(&lua, key_generators.clone());
        Self::inject_json(&lua);

        lua.load(script).exec().expect("Script failed to load");
        let cmd: LuaFunction = lua.globals().get("generate").expect("generate function not found");

        (lua, cmd)
    }

    fn inject_bench(lua: &Lua, key_generators: SharedKeyGenerator) {
        // Create bench object
        let bench = lua.create_table().expect("Failed to create bench table");

        // Add key function to bench object
        let key_func = lua
            .create_function(move |lua, (range, distribution, name): (u64, String, String)| {
                // Check if we have a named generator
                let mut generators = key_generators.lock().unwrap();
                let key_gen = if let Some(existing_gen) = generators.get(&name) {
                    // Clone existing generator
                    existing_gen.clone()
                } else {
                    // Create new generator and store it
                    let new_gen = KeyGenerator::new(&distribution, range);
                    generators.insert(name, new_gen.clone());
                    new_gen
                };

                lua.create_userdata(key_gen)
            })
            .expect("Failed to create key function");

        // Add value function to bench object (no naming support, creates new instance each time)
        let value_func = lua.create_function(|lua, size: u64| lua.create_userdata(ValueGenerator::new(size))).expect("Failed to create value function");

        // Add rand function to bench object (no naming support, creates new instance each time)
        let rand_func = lua.create_function(|lua, range: u64| lua.create_userdata(RandGenerator::new(range))).expect("Failed to create rand function");

        bench.set("key", key_func).expect("Failed to set key function");
        bench.set("value", value_func).expect("Failed to set value function");
        bench.set("rand", rand_func).expect("Failed to set rand function");
        lua.globals().set("bench", bench).expect("Failed to set bench global");
    }

    fn inject_json(lua: &Lua) {
        let json = lua.create_table().expect("Failed to create json table");

        let encode_func = lua
            .create_function(|_, val: mlua::Value| serde_json::to_string(&val).map_err(|e| mlua::Error::RuntimeError(format!("JSON encode error: {}", e))))
            .expect("Failed to create encode function");

        json.set("encode", encode_func).expect("Failed to set encode function");

        lua.globals().set("json", json).expect("Failed to set json global");
    }
}

impl Clone for LuaGenerator {
    fn clone(&self) -> Self {
        let (lua, cmd) = LuaGenerator::setup_lua_vm(&self.script, self.key_generators.clone());
        LuaGenerator {
            script: self.script.clone(),
            lua,
            cmd,
            key_generators: self.key_generators.clone(),
        }
    }
}

impl CommandGenerator for LuaGenerator {
    fn gen_cmd(&mut self) -> redis::Cmd {
        let args = self.cmd.call::<Vec<String>>(()).expect("generate function failed");
        let mut cmd: redis::Cmd = redis::Cmd::new();
        for arg in args {
            cmd.arg(arg);
        }
        cmd
    }

    fn gen_cmd_with_lock(&mut self) -> redis::Cmd {
        self.gen_cmd()
    }

    fn clone_box(&self) -> Box<dyn CommandGenerator> {
        Box::new(self.clone())
    }
}

impl std::string::ToString for LuaGenerator {
    fn to_string(&self) -> String {
        self.script.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_redis_args(cmd: &redis::Cmd) -> Vec<String> {
        cmd.args_iter()
            .map(|arg| match arg {
                redis::Arg::Simple(s) => String::from_utf8(s.to_vec()).unwrap(),
                redis::Arg::Cursor => "Cursor".into(),
            })
            .collect()
    }

    #[test]
    fn test_lua_common() {
        let script = r#"
        function generate()
            return {"hello"}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args, vec!["hello"]);
    }

    #[test]
    fn test_lua_global() {
        let script = r#"
        global_count = 0
        function generate()
            global_count = global_count + 1
            return {"set", "key", global_count}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args, vec!["set", "key", "1"]);
        let args: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args, vec!["set", "key", "2"]);
    }

    #[test]
    fn test_bench_key_uniform() {
        let script = r#"
        local key_gen = bench.key(100, "uniform", "my_key")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args1.len(), 3);
        assert_eq!(args1[0], "set");
        assert_eq!(args1[2], "value");
        assert!(args1[1].starts_with("key:"));
        assert!(args2[1].starts_with("key:"));
    }

    #[test]
    fn test_bench_key_sequence() {
        let script = r#"
        local key_gen = bench.key(10, "sequence", "my_seq")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args1[1], "key:0");
        assert_eq!(args2[1], "key:1");
    }

    #[test]
    fn test_bench_key_zipfian() {
        let script = r#"
        local key_gen = bench.key(100, "zipfian", "my_zipf")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args1.len(), 3);
        assert_eq!(args1[0], "set");
        assert_eq!(args1[2], "value");
        assert!(args1[1].starts_with("key:"));
        assert!(args2[1].starts_with("key:"));
    }

    #[test]
    fn test_bench_key_call_syntax() {
        let script = r#"
        local key_gen = bench.key(10, "sequence", "my_seq_call")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());
        assert_eq!(args1[1], "key:0");
        assert_eq!(args2[1], "key:1");
    }

    #[test]
    fn test_bench_value_generator() {
        let script = r#"
        local value_gen = bench.value(10)
        function generate()
            return {"set", "key", value_gen()}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());

        // Both should be 10-character alphanumeric strings
        assert_eq!(args1.len(), 3);
        assert_eq!(args1[0], "set");
        assert_eq!(args1[1], "key");
        assert_eq!(args1[2].len(), 10);
        assert_eq!(args2[2].len(), 10);

        // They should be different (random)
        assert_ne!(args1[2], args2[2]);
    }

    #[test]
    fn test_bench_rand_generator() {
        let script = r#"
        local rand_gen = bench.rand(100)
        function generate()
            return {"set", "key", tostring(rand_gen())}
        end
        "#;
        let mut lua = LuaGenerator::new(script);
        let args1: Vec<String> = extract_redis_args(&lua.gen_cmd());
        let args2: Vec<String> = extract_redis_args(&lua.gen_cmd());

        // Both should be numbers in range [0, 100)
        assert_eq!(args1.len(), 3);
        assert_eq!(args1[0], "set");
        assert_eq!(args1[1], "key");

        let num1: u64 = args1[2].parse().unwrap();
        let num2: u64 = args2[2].parse().unwrap();
        assert!(num1 < 100);
        assert!(num2 < 100);

        // They should usually be different (random)
        // Note: there's a small chance they could be the same, but very unlikely
        // We'll just verify they are valid numbers in range
    }

    #[test]
    fn test_bench_key_named_sequence_clone() {
        let script = r#"
        local key_gen = bench.key(10, "sequence", "shared_seq")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;

        // Create one LuaGenerator instance and clone it
        let mut lua1 = LuaGenerator::new(script);
        let mut lua2 = lua1.clone();

        // Both should share the same sequence state because they share the same key_generators
        let args1_1: Vec<String> = extract_redis_args(&lua1.gen_cmd());
        let args2_1: Vec<String> = extract_redis_args(&lua2.gen_cmd());
        let args1_2: Vec<String> = extract_redis_args(&lua1.gen_cmd());
        let args2_2: Vec<String> = extract_redis_args(&lua2.gen_cmd());

        // Should get sequential values: 0, 1, 2, 3
        assert_eq!(args1_1[1], "key:0");
        assert_eq!(args2_1[1], "key:1");
        assert_eq!(args1_2[1], "key:2");
        assert_eq!(args2_2[1], "key:3");
    }

    #[test]
    fn test_clone_creates_independent_lua_vm() {
        let script = r#"
        -- Set a global variable to test VM independence
        if not initialized then
            global_counter = 0
            initialized = true
        end
        local key_gen = bench.key(10, "sequence", "test_seq")
        function generate()
            global_counter = global_counter + 1
            return {"set", "key:" .. key_gen(), "counter:" .. global_counter}
        end
        "#;

        // Create one LuaGenerator instance and clone it
        let mut lua1 = LuaGenerator::new(script);
        let mut lua2 = lua1.clone();

        // Each should have its own global_counter starting from 0
        let args1_1: Vec<String> = extract_redis_args(&lua1.gen_cmd());
        let args2_1: Vec<String> = extract_redis_args(&lua2.gen_cmd());

        // Both should start with counter:1 (since global_counter starts at 0, then increments to 1)
        assert_eq!(args1_1[2], "counter:1");
        assert_eq!(args2_1[2], "counter:1");

        // But they should share the same key generator sequence
        assert_eq!(args1_1[1], "key:0");
        assert_eq!(args2_1[1], "key:1");
    }

    #[test]
    fn test_bench_key_different_names() {
        let script1 = r#"
        local key_gen = bench.key(10, "sequence", "seq1")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;
        let script2 = r#"
        local key_gen = bench.key(10, "sequence", "seq2")
        function generate()
            return {"set", "key:" .. key_gen(), "value"}
        end
        "#;

        // Create two separate LuaGenerator instances with different names
        let mut lua1 = LuaGenerator::new(script1);
        let mut lua2 = LuaGenerator::new(script2);

        // Should have independent sequences
        let args1_1: Vec<String> = extract_redis_args(&lua1.gen_cmd());
        let args2_1: Vec<String> = extract_redis_args(&lua2.gen_cmd());
        let args1_2: Vec<String> = extract_redis_args(&lua1.gen_cmd());
        let args2_2: Vec<String> = extract_redis_args(&lua2.gen_cmd());

        // Each should start from 0 independently
        assert_eq!(args1_1[1], "key:0");
        assert_eq!(args2_1[1], "key:0");
        assert_eq!(args1_2[1], "key:1");
        assert_eq!(args2_2[1], "key:1");
    }
}
