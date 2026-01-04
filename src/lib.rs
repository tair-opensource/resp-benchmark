mod async_flag;
mod auto_connection;
mod bench;
mod client;
mod command;
mod histogram;
mod shared_context;

use crate::command::{LuaGenerator, PlaceholderGenerator};
use ctrlc;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

/// A Python module implemented in Rust.
#[pymodule]
fn _resp_benchmark_rust_lib(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(benchmark, m)?)?;
    Ok(())
}

#[pyclass]
#[derive(Default, Copy, Clone)]
struct BenchmarkResult {
    #[pyo3(get, set)] pub qps: f64,
    #[pyo3(get, set)] pub avg_latency_ms: f64,
    #[pyo3(get, set)] pub p99_latency_ms: f64,
    #[pyo3(get, set)] pub connections: u64,
}

#[pyfunction]
fn benchmark(
    py: Python<'_>,
    host: String,
    port: u16,
    username: String,
    password: String,
    cluster: bool,
    tls: bool,
    timeout: u64,
    cores: Vec<u16>,
    command: String,
    connections: u64,
    target: u64,
    pipeline: u64,
    count: u64,
    seconds: u64,
    load: bool,
    quiet: bool,
    short_connection: bool,
    use_lua: bool,
) -> PyResult<BenchmarkResult> {
    assert!(cores.len() > 0);
    if load {
        assert_ne!(count, 0, "count must be greater than 0");
    }
    if short_connection {
        assert!(!load, "short_connection mode does not support loading");
        assert_eq!(pipeline, 1, "short_connection mode does not support pipeline (pipeline must be 1)");
    }

    let _ = ctrlc::set_handler(move || {
        std::process::exit(0);
    });

    let client_config = client::ClientConfig {
        cluster,
        address: format!("{}:{}", host, port),
        username,
        password,
        tls,
        timeout,
    };
    let case = bench::Case {
        command: if use_lua { Box::new(LuaGenerator::new(command.as_str())) } else { Box::new(PlaceholderGenerator::new(command.as_str())) },
        connections,
        pipeline,
        count,
        target,
        seconds,
        short_connection,
    };
    let result = py.allow_threads(|| bench::do_benchmark(client_config, cores, case, load, quiet));
    Ok(result)
}
