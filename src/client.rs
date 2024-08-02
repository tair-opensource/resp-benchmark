use enum_delegate;
use redis::aio::ConnectionLike;
use redis::aio::MultiplexedConnection;
use redis::cluster_async::ClusterConnection;
use redis::{Cmd, RedisFuture, Value};
use std::fmt::{Display, Formatter};

#[derive(Clone)]
pub struct ClientConfig {
    pub cluster: bool,
    pub address: String,
    pub username: String,
    pub password: String,
    pub tls: bool,
    pub timeout: u64,
}

impl ClientConfig {
    pub async fn get_client(&self) -> Client {
        let conn_str = if self.tls {
            format!("rediss://{}:{}@{}/#insecure", &self.username, &self.password, &self.address)
        } else {
            format!("redis://{}:{}@{}", &self.username, &self.password, &self.address)
        };

        if self.cluster {
            let nodes = vec![conn_str];
            let client = redis::cluster::ClusterClient::builder(nodes).connection_timeout(std::time::Duration::from_secs(self.timeout)).build().unwrap();
            let conn = match client.get_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", self.address, e);
                    std::process::exit(1);
                }
            };
            Client::new(conn.into())
        } else {
            let client = redis::Client::open(conn_str).unwrap();
            let conn = match client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", self.address, e);
                    std::process::exit(1);
                }
            };
            Client::new(conn.into())
        }
    }
}

impl Display for ClientConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RedisConfig {{ cluster: {}, address: {}, username: {}, password: {}, tls: {} }}", self.cluster, self.address, self.username, self.password, self.tls)
    }
}

#[enum_delegate::implement(ConnectionLike,
trait ConnectionLike {
fn req_packed_command < 'a > (& 'a mut self, cmd: & 'a Cmd) -> RedisFuture < 'a, Value >;
fn req_packed_commands < 'a > (
& 'a mut self,
cmd: & 'a redis::Pipeline,
offset: usize,
count: usize,
) -> RedisFuture < 'a, Vec < Value >>;
fn get_db(& self) -> i64;
}
)]
enum ClientConnection {
    Standalone(MultiplexedConnection),
    Cluster(ClusterConnection),
}

pub struct Client {
    conn: ClientConnection,
}

impl Client {
    fn new(conn: ClientConnection) -> Client {
        Client { conn }
    }


    pub async fn run_commands(&mut self, cmds: Vec<redis::Cmd>) {
        let mut pipeline = redis::pipe();
        for cmd in cmds {
            pipeline.add_command(cmd).ignore();
        }
        match pipeline.query_async(&mut self.conn).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to execute pipeline: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}
