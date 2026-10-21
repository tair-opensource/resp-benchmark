use awaitgroup::WaitGroup;
use colored::Colorize;
use std::io::Write;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::{select, task};

use crate::BenchmarkResult;
use crate::auto_connection::{AutoConnection, ConnLimiter};
use crate::client::{Client, ClientConfig};
use crate::command::CommandGenerator;
use crate::shared_context::SharedContext;
use governor::{DefaultDirectRateLimiter, Jitter, Quota};

pub struct Case {
    pub command: Box<dyn CommandGenerator>,
    pub connections: u64,
    pub count: u64,
    pub target: u64,
    pub seconds: u64,
    pub pipeline: u64,
    pub short_connection: bool,
}

impl Clone for Case {
    fn clone(&self) -> Self {
        Self {
            command: self.command.clone_box(),
            connections: self.connections,
            count: self.count,
            target: self.target,
            seconds: self.seconds,
            pipeline: self.pipeline,
            short_connection: self.short_connection,
        }
    }
}

async fn run_commands_on_single_thread(conn_limiter: Arc<ConnLimiter>, qps_limiter: Arc<Option<DefaultDirectRateLimiter>>, config: ClientConfig, case: Case, context: SharedContext) {
    let local = task::LocalSet::new();
    for _ in 0..conn_limiter.total_conn {
        let conn_limiter = conn_limiter.clone();
        let qps_limiter = qps_limiter.clone();
        let config = config.clone();
        let case = case.clone();
        let mut context = context.clone();
        local.spawn_local(async move {
            let mut cmd = case.command.clone_box();
            let conn_limiter = conn_limiter.clone();
            select! {
                _ = conn_limiter.wait_new_conn() =>{}
                _ = context.wait_stop() => {
                    return;
                }
            }

            let mut client: Option<Client> = None;
            let mut database = None;
            loop {
                let db = match database.or_else(|| context.next_database()) {
                    Some(db) => db,
                    None => {
                        context.stop();
                        break;
                    }
                };
                let pipeline_cnt = context.fetch(db, case.pipeline);
                if pipeline_cnt == 0 {
                    // This database is exhausted. Reuse the connection for another
                    // database so even a single worker can finish all quotas.
                    database = None;
                    continue;
                }

                if let Some(limiter) = qps_limiter.as_ref() {
                    let j = Jitter::up_to(std::time::Duration::from_micros(1_000_000_u64 * conn_limiter.total_conn / case.target));
                    limiter.until_ready_with_jitter(j).await;
                }

                if database != Some(db) {
                    match client.as_mut() {
                        Some(client) => client.select_database(config.dbids[db]).await,
                        None => {
                            // The Redis client selects the database during connection setup.
                            client = Some(config.get_client(config.dbids[db]).await);
                        }
                    }
                    database = Some(db);
                }

                let mut p = Vec::new();
                for _ in 0..pipeline_cnt {
                    if context.is_loading {
                        p.push(cmd.gen_cmd_with_lock());
                    } else {
                        p.push(cmd.gen_cmd());
                    }
                }
                let instant = std::time::Instant::now();
                let should_count = client.as_mut().unwrap().run_commands(p).await;
                let duration = instant.elapsed().as_micros() as u64;

                if case.short_connection {
                    client = None;
                    database = None;
                }
                if should_count {
                    for _ in 0..pipeline_cnt {
                        context.histogram.record(duration);
                    }
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
                    std::io::stdout().flush().unwrap();
                }
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
        if !quiet {
            if context.is_loading {
                println!("\x1B[F\x1B[2KData loaded, qps: {:.0}, time elapsed: {:.2}s\n", result.qps, overall_time.elapsed().as_secs_f64());
            } else {
                println!("\x1B[F\x1B[2Kqps: {:.0}, conn: {}, {}\n", result.qps, conn, histogram)
            };
        }
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
        println!("{}: {}", "target".bold().blue(), if case.target == 0 { "unlimited".to_string() } else { case.target.to_string() });
        println!("{}: {}", "seconds".bold().blue(), case.seconds);
        println!("{}: {}", "pipeline".bold().blue(), case.pipeline);
        println!("{}: {}", "short_connection".bold().blue(), case.short_connection);
        println!("{}: {:?}", "dbids".bold().blue(), client_config.dbids);
    }

    // calc connections
    let auto_connection = AutoConnection::new(case.connections, cores.len() as u64);

    // calc target qps
    let qps_limiter = Arc::new(if case.target > 0 {
        let limiter = DefaultDirectRateLimiter::direct(Quota::per_second(NonZeroU32::new(case.target as u32).unwrap()));
        while limiter.check().is_ok() {}
        Some(limiter)
    } else {
        None
    });

    let mut thread_handlers = Vec::new();
    let wg = WaitGroup::new();
    let core_ids = core_affinity::get_core_ids().unwrap();
    let context = SharedContext::new(case.count, case.seconds, load, client_config.dbids.len());
    for inx in 0..cores.len() {
        let client_config = client_config.clone();
        let case = case.clone();
        let context = context.clone();
        let wk = wg.worker();
        let core_id = core_ids[cores[inx] as usize];
        let conn_limiter = auto_connection.limiters[inx].clone();
        let qps_limiter = qps_limiter.clone();
        let thread_handler = std::thread::spawn(move || {
            core_affinity::set_for_current(core_id); // not work on Apple Silicon
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                run_commands_on_single_thread(conn_limiter, qps_limiter, client_config, case, context).await;
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
