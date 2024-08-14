use std::io::Write;
use std::sync::Arc;
use awaitgroup::WaitGroup;
use colored::Colorize;
use tokio::{select, task};

use crate::BenchmarkResult;
use crate::client::ClientConfig;
use crate::command::Command;
use crate::auto_connection::{AutoConnection, ConnLimiter};
use crate::shared_context::SharedContext;

#[derive(Clone)]
pub struct Case {
    pub command: Command,
    pub connections: u64,
    pub count: u64,
    pub seconds: u64,
    pub pipeline: u64,
}

async fn run_commands_on_single_thread(limiter: Arc<ConnLimiter>, config: ClientConfig, case: Case, context: SharedContext) {
    let local = task::LocalSet::new();
    for _ in 0..limiter.total_conn {
        let limiter = limiter.clone();
        let config = config.clone();
        let case = case.clone();
        let mut context = context.clone();
        local.spawn_local(async move {
            let mut client = config.get_client().await;
            let mut cmd = case.command.clone();
            let limiter = limiter.clone();
            select! {
                _ = limiter.wait_new_conn() =>{}
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
                    if context.is_loading {
                        p.push(cmd.gen_cmd_with_lock());
                    } else {
                        p.push(cmd.gen_cmd());
                    }
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

fn wait_finish(case: &Case, mut auto_connection: AutoConnection, mut context: SharedContext, mut wg: WaitGroup, quiet: bool) -> BenchmarkResult {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let mut result = BenchmarkResult::default();

    rt.block_on(async {
        let histogram = context.histogram.clone();
        // calc overall qps
        let mut overall_time = std::time::Instant::now();
        let mut overall_cnt_overhead = 0;
        // for log
        let mut log_instance = std::time::Instant::now();
        let mut log_last_cnt = histogram.cnt();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(233));

        if auto_connection.ready {
            context.start_timer();
            overall_time = std::time::Instant::now();
            overall_cnt_overhead = 0;
        }

        loop {
            select! {
                _ = interval.tick() => {}
                _ = wg.wait() => {break;}
            }
            {
                let cnt = histogram.cnt();
                let qps = (cnt - log_last_cnt) as f64 / log_instance.elapsed().as_secs_f64();
                let conn: u64 = auto_connection.active_conn();
                if auto_connection.ready {
                    result.qps = (cnt - overall_cnt_overhead) as f64 / overall_time.elapsed().as_secs_f64();
                }
                if !quiet {
                    if context.is_loading {
                        println!("\x1B[F\x1B[2KData loading qps: {:.0}, {:.2}%", qps, histogram.cnt() as f64 / case.count as f64 * 100f64);
                    } else {
                        println!("\x1B[F\x1B[2Kqps: {:.0}(overall {:.0}), conn: {}, {}", qps, result.qps, conn, histogram);
                    }
                }
                std::io::stdout().flush().unwrap();
                log_last_cnt = cnt;
                log_instance = std::time::Instant::now();
            }
            if !auto_connection.ready {
                auto_connection.adjust(&histogram);
                if auto_connection.ready {
                    overall_cnt_overhead = histogram.cnt();
                    overall_time = std::time::Instant::now();
                    context.start_timer();
                }
            }
        }
        let conn: u64 = auto_connection.active_conn();
        if context.is_loading {
            println!("\x1B[F\x1B[2KData loaded, qps: {:.0}, time elapsed: {:.2}s\n", result.qps, overall_time.elapsed().as_secs_f64());
        } else {
            println!("\x1B[F\x1B[2Kqps: {:.0}, conn: {}, {}\n", result.qps, conn, histogram)
        };
        result.avg_latency_ms = histogram.avg() as f64 / 1_000.0;
        result.p99_latency_ms = histogram.percentile(0.99) as f64 / 1_000.0;
        result.connections = conn;
    });
    return result;
}

pub fn do_benchmark(client_config: ClientConfig, cores: Vec<u16>, case: Case, load: bool, quiet: bool) -> BenchmarkResult {
    if !quiet {
        println!("{}: {}", "command".bold().blue(), case.command.to_string().green().bold());
        println!("{}: {}", "connections".bold().blue(), if case.connections == 0 { "auto".to_string() } else { case.connections.to_string() });
        println!("{}: {}", "count".bold().blue(), case.count);
        println!("{}: {}", "seconds".bold().blue(), case.seconds);
        println!("{}: {}", "pipeline".bold().blue(), case.pipeline);
    }

    // calc connections
    let auto_connection = AutoConnection::new(case.connections, cores.len() as u64);

    let mut thread_handlers = Vec::new();
    let wg = WaitGroup::new();
    let core_ids = core_affinity::get_core_ids().unwrap();
    let context = SharedContext::new(case.count, case.seconds, load);
    for inx in 0..cores.len() {
        let client_config = client_config.clone();
        let case = case.clone();
        let context = context.clone();
        let wk = wg.worker();
        let core_id = core_ids[cores[inx] as usize];
        let limiter = auto_connection.limiters[inx].clone();
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
    let result = wait_finish(&case, auto_connection, context, wg, quiet);

    // join all threads
    for thread_handler in thread_handlers {
        loop {
            if thread_handler.is_finished() {
                thread_handler.join().unwrap();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    return result;
}