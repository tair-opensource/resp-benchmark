mod client;
mod command;
mod common;
mod config;
mod histogram;
mod limiter;
mod output;
mod shared_context;

use crate::client::ClientConfig;
use crate::config::{Case, Dataset};
use crate::limiter::{AutoLimiter, Limiter};
use crate::shared_context::SharedContext;
use awaitgroup::WaitGroup;
use colored::Colorize;
use core_affinity;
pub use histogram::Histogram;
use std::cmp::min;
use std::io::Write;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::{select, task};

async fn load_data(config: ClientConfig, dataset: Dataset, total_count: Arc<AtomicU64>, conn_per_thread: usize) {
    let mut tasks = JoinSet::new();
    let pipeline_count = if config.cluster { 1 } else { 8 };
    for inx in 0..conn_per_thread {
        let config = config.clone();
        let mut cmd = dataset.command.clone();
        let total_count = total_count.clone();
        let dataset = dataset.clone();
        tasks.spawn(async move {
            let mut client = config.get_client().await;
            loop {
                let prev_count = total_count.fetch_add(pipeline_count, std::sync::atomic::Ordering::SeqCst);
                if prev_count >= dataset.count {
                    break;
                }
                if inx == 0 && prev_count % 1007 == 0 {
                    print!("\r\x1B[2K{}: {} {}/{}", "dataset loading".blue().bold(), dataset.command.to_string().green().bold(), prev_count, dataset.count);
                    std::io::stdout().flush().unwrap();
                }
                let pipeline_cnt = min(dataset.count - prev_count, pipeline_count);
                let mut pipeline = Vec::new();
                for _ in 0..pipeline_cnt {
                    pipeline.push(cmd.gen_cmd_with_lock());
                }
                client.run_commands(pipeline).await;
            }
        });
    }
    for _ in 0..conn_per_thread {
        tasks.join_next().await;
    }
}

async fn run_commands_on_single_thread(limiter: Arc<Limiter>, config: ClientConfig, case: Case, context: SharedContext) {
    let local = task::LocalSet::new();
    for _ in 0..limiter.tcp_conn {
        let limiter = limiter.clone();
        let config = config.clone();
        let case = case.clone();
        let mut context = context.clone();
        local.spawn_local(async move {
            let mut client = config.get_client().await;
            let mut cmd = case.command.clone();
            let limiter = limiter.clone();
            select! {
                _ = limiter.wait_add() =>{}
                _ = context.wait_stop() => {
                    return;
                }
            }
            loop {
                let pipeline_cnt = context.fetch(case.pipeline);
                if pipeline_cnt == 0 {
                    context.stop();
                    break;
                }

                // prepare pipeline
                let mut p = Vec::new();
                for _ in 0..pipeline_cnt {
                    p.push(cmd.gen_cmd());
                }
                let instant = std::time::Instant::now();
                client.run_commands(p).await;
                let duration = instant.elapsed().as_micros() as u64;
                for _ in 0..pipeline_cnt {
                    context.histogram.record(duration);
                }
            }
        });
    }
    local.await;
}

fn wait_finish(case: &Case, client_config: &ClientConfig, mut auto_limiter: AutoLimiter, mut context: SharedContext, mut wg: WaitGroup) -> output::Result {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let mut result = output::Result::default();
    result.name = case.name.clone().unwrap_or(case.command.to_string());
    rt.block_on(async {
        let histogram = context.histogram.clone();
        let mut client = client_config.get_client().await;
        // calc overall qps
        let mut overall_time = std::time::Instant::now();
        let mut overall_cnt_overhead = 0;
        // for log
        let mut log_instance = std::time::Instant::now();
        let mut log_last_cnt = histogram.cnt();
        let mut log_interval = tokio::time::interval(std::time::Duration::from_millis(100));
        // for auto limiter
        let mut auto_interval = tokio::time::interval(std::time::Duration::from_secs(10));

        let mut wake_log = false;
        let mut wake_auto = false;
        loop {
            select! {
                _ = log_interval.tick() => {wake_log=true;}
                _ = auto_interval.tick() => {wake_auto=true;}
                _ = wg.wait() => {break;}
            }
            if wake_log {
                wake_log = false;
                result.memory = client.info_memory().await as f64 / 1024.0 / 1024.0 / 1024.0;
                let cnt = histogram.cnt();
                let qps = (cnt - log_last_cnt) as f64 / log_instance.elapsed().as_secs_f64();
                let active_conn: u64 = auto_limiter.active_conn();
                let target_conn: u64 = auto_limiter.target_conn();
                if auto_limiter.ready {
                    result.qps = (cnt - overall_cnt_overhead) as f64 / overall_time.elapsed().as_secs_f64();
                }
                print!("\r\x1B[2Kqps: {:.0}(overall {:.0}), active_conn: {}, target_conn: {}, {}, mem: {:.3}GiB", qps, result.qps, active_conn, target_conn, histogram, result.memory);
                std::io::stdout().flush().unwrap();
                log_last_cnt = cnt;
                log_instance = std::time::Instant::now();
            }
            if wake_auto && !auto_limiter.ready {
                wake_auto = false;

                let cnt = histogram.cnt();
                if !auto_limiter.adjust(cnt) {
                    overall_cnt_overhead = histogram.cnt();
                    overall_time = std::time::Instant::now();
                    context.start_timer();
                }
            }
        }
        let active_conn: u64 = auto_limiter.active_conn();
        print!("\r\x1B[2Kqps: {:.0}, conn: {}, {}\n", result.qps, active_conn, histogram);
        result.avg = histogram.avg() as f64 / 1_000.0;
        result.min = histogram.percentile(0.0) as f64 / 1_000.0;
        result.p50 = histogram.percentile(0.5) as f64 / 1_000.0;
        result.p90 = histogram.percentile(0.9) as f64 / 1_000.0;
        result.p99 = histogram.percentile(0.99) as f64 / 1_000.0;
        result.max = histogram.percentile(1.0) as f64 / 1_000.0;
        result.connections = active_conn;
        result.pipeline = case.pipeline;
        result.count = histogram.cnt();
        result.duration = overall_time.elapsed().as_secs_f64();
    });
    return result;
}

fn main() {
    let conf = config::Config::parse();
    println!("{}: {:?}", "CPU Core".bold(), conf.cpus);
    println!("{}: {}", "Address".bold(), conf.client_config.address);
    println!("{}: {}", "Cluster".bold(), conf.client_config.cluster);
    println!("{}: {}", "TLS".bold(), conf.client_config.tls);
    println!("{}: {:?}", "output".bold(), conf.output_formats);

    // check cpu core
    let core_ids = core_affinity::get_core_ids().unwrap();
    for cpu_id in conf.cpus.iter() {
        if *cpu_id as usize >= core_ids.len() {
            // print red
            println!("{}: {:?}", "Invalid CPU core ids".bold().red(), conf.cpus);
            std::process::exit(1);
        }
    }

    let mut results = output::Results::new();
    for case in conf.cases.iter() {
        println!();
        if let Some(name) = case.name.clone() {
            println!("{}: {}", "name".bold().blue(), name.bold());
        }

        // flushall
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut client = conf.client_config.get_client().await;
            client.flushall().await;
        });

        if let Some(dataset) = &case.dataset {
            let mut thread_handlers = Vec::new();
            let core_ids = core_affinity::get_core_ids().unwrap();
            let count = Arc::new(AtomicU64::new(0));
            let conn_per_thread = if cfg!(target_os = "macos") { 4 } else { min(64, 512 / conf.cpus.len()) };
            for cpu_id in conf.cpus.iter() {
                let core_id = core_ids[*cpu_id as usize];
                let redis_config = conf.client_config();
                let dataset = dataset.clone();
                let count = count.clone();
                let thread_handler = std::thread::spawn(move || {
                    core_affinity::set_for_current(core_id); // not work on Apple Silicon
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    rt.block_on(async {
                        load_data(redis_config, dataset, count, conn_per_thread).await;
                    });
                });
                thread_handlers.push(thread_handler);
            }
            for thread_handler in thread_handlers {
                thread_handler.join().unwrap();
            }
            println!("\r\x1B[2K{}: {} x {}", "dataset".blue().bold(), dataset.command.to_string().bold(), dataset.count);
        }

        println!("{}: {}", "command".bold().blue(), case.command.to_string().green().bold());
        println!("{}: {}", "connections".bold().blue(), if case.connections == 0 { "auto".to_string() } else { case.connections.to_string() });
        println!("{}: {}", "count".bold().blue(), case.count);
        println!("{}: {}", "pipeline".bold().blue(), case.pipeline);

        // calc connections
        let auto_limiter = AutoLimiter::new(case.connections, conf.cpus.len() as u64);

        let mut thread_handlers = Vec::new();
        let wg = WaitGroup::new();
        let core_ids = core_affinity::get_core_ids().unwrap();
        let context = SharedContext::new(case.count, case.seconds);
        for inx in 0..conf.cpus.len() {
            let client_config = conf.client_config().clone();
            let case = case.clone();
            let context = context.clone();
            let wk = wg.worker();
            let core_id = core_ids[conf.cpus.get(inx) as usize];
            let limiter = auto_limiter.limiters[inx].clone();
            let thread_handler = std::thread::spawn(move || {
                core_affinity::set_for_current(core_id); // not work on Apple Silicon
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                rt.block_on(async {
                    run_commands_on_single_thread(limiter, client_config, case, context).await;
                    wk.done();
                });
            });
            thread_handlers.push(thread_handler);
        }

        // log thread
        let result = wait_finish(&case, &conf.client_config(), auto_limiter, context, wg);
        results.add(result);

        // join all threads
        for thread_handler in thread_handlers {
            thread_handler.join().unwrap();
        }
    }

    results.save(conf.output_formats);
}
