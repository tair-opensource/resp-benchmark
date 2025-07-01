mod bench;
mod client;
mod command;
mod auto_connection;
mod shared_context;
mod histogram;
mod qps_limiter;
mod async_flag;

use ctrlc;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use crate::command::Command;
use crate::shared_context::SharedContext;

/// A Python module implemented in Rust.
#[pymodule]
fn _resp_benchmark_rust_lib(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(benchmark, m)?)?;
    m.add_function(wrap_pyfunction!(async_benchmark, m)?)?;
    m.add_class::<BenchmarkWorkerPool>()?;
    Ok(())
}


#[pyclass]
#[derive(Default, Copy, Clone)]
struct BenchmarkResult {
    #[pyo3(get, set)] pub qps: f64,
    #[pyo3(get, set)] pub avg_latency_ms: f64,
    #[pyo3(get, set)] pub p99_latency_ms: f64,
    #[pyo3(get, set)] pub max_latency_ms: f64,
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
) -> PyResult<BenchmarkResult> {
    assert!(cores.len() > 0);
    if load {
        assert_ne!(count, 0, "count must be greater than 0");
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
        command: Command::new(command.as_str()),
        connections,
        pipeline,
        count,
        target,
        seconds,
    };
    let result = py.allow_threads(|| bench::do_benchmark(client_config, cores, case, load, quiet));
    Ok(result)
}

#[pyclass]
struct BenchmarkWorkerPool {
    pool: std::sync::Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl BenchmarkWorkerPool {
    #[new]
    fn new() -> PyResult<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(Self { pool: std::sync::Arc::new(rt) })
    }
}

#[pyclass]
pub struct AsyncBenchmarkContext {
    ctx: SharedContext,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

#[pymethods]
impl AsyncBenchmarkContext {
    fn stop(&mut self) {
        self.ctx.stop();
    }

    fn join(&mut self) {
        match self.join_handle.take() {
            None => {}
            Some(handle) => {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                let _ = rt.block_on(handle);
            }
        }
    }

    fn try_join(&mut self) -> bool {
        self.join_handle.as_ref().map_or(true, |v| v.is_finished())
    }

    fn current_result(&self) -> BenchmarkResult { 
        let result = self.ctx.latest_result.lock().unwrap().clone();
        return result;
    }
}

#[pyfunction]
fn async_benchmark(
    worker_pool: &BenchmarkWorkerPool,
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
) -> PyResult<AsyncBenchmarkContext> {
    if load {
        return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("count must be greater than 0".to_string()));
    }

    let client_config = client::ClientConfig {
        cluster,
        address: format!("{}:{}", host, port),
        username,
        password,
        tls,
        timeout,
    };
    let case = bench::Case {
        command: Command::new(command.as_str()),
        connections,
        pipeline,
        count,
        target,
        seconds,
    };
    let pool = worker_pool.pool.clone();
    let result = bench::do_benchmark_async(pool, client_config, cores, case, load, quiet);
    Ok(result)
}