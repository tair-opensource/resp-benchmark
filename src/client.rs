use enum_delegate;
use redis::aio::ConnectionLike;
use redis::aio::MultiplexedConnection;
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{RoutingInfo, SingleNodeRoutingInfo};
use redis::{from_redis_value, Cmd, InfoDict, RedisFuture, Value};
use serde::Deserialize;
use std::fmt::{Display, Formatter};
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DBType {
    // https://github.com/redis/redis
    #[default]
    Redis,

    // https://github.com/valkey-io/valkey
    Valkey,

    // https://www.alibabacloud.com/product/tair
    #[serde(rename = "tair_mem")]
    TairMem,
    #[serde(rename = "tair_scm")]
    TairScm,
    #[serde(rename = "tair_ssd")]
    TairSsd,

    Garnet,

    // Others
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    #[serde(default)]
    pub cluster: bool,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub db_type: DBType,
    #[serde(default)]
    replica_count: u64,
}

impl ClientConfig {
    pub async fn get_client(&self) -> Client {
        let conn_str = if self.tls {
            format!("rediss://{}:{}@{}/#insecure", &self.username, &self.password, &self.address)
        } else {
            format!("redis://{}:{}@{}", &self.username, &self.password, &self.address)
        };
        return if self.cluster {
            let nodes = vec![conn_str];
            let client = redis::cluster::ClusterClient::builder(nodes).connection_timeout(std::time::Duration::from_secs(5)).build().unwrap();
            let conn = match client.get_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", self.address, e);
                    std::process::exit(1);
                }
            };
            Client::new(conn.into(), self.db_type.clone(), self.replica_count)
        } else {
            let client = redis::Client::open(conn_str).unwrap();
            let conn = match client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", self.address, e);
                    std::process::exit(1);
                }
            };
            Client::new(conn.into(), self.db_type.clone(), self.replica_count)
        };
    }
}

impl Display for ClientConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RedisConfig {{ cluster: {}, address: {}, username: {}, password: {}, tls: {} }}", self.cluster, self.address, self.username, self.password, self.tls)
    }
}

#[enum_delegate::implement(ConnectionLike,
    trait ConnectionLike {
        fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value>;
        fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a redis::Pipeline,
        offset: usize,
        count: usize,
        ) -> RedisFuture<'a, Vec<Value>>;
        fn get_db(&self) -> i64;
    }
)]
enum ClientConnection {
    Standalone(MultiplexedConnection),
    Cluster(ClusterConnection),
}

pub struct Client {
    conn: ClientConnection,
    db_type: DBType,
    replica_count: u64,
}

impl Client {
    fn new(conn: ClientConnection, db_type: DBType, replica_count: u64) -> Client {
        Client { conn, db_type, replica_count }
    }

    pub async fn flushall(&mut self) {
        // flushall
        let cmd = {
            let mut cmd = Cmd::new();
            match &self.db_type {
                DBType::Garnet => cmd.arg("FLUSHDB"),
                DBType::TairScm => cmd.arg("FLUSHALL"),
                _ => cmd.arg("FLUSHALL"),
            };
            cmd
        };
        match cmd.query_async(&mut self.conn).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to execute `flushall` command: {:?}", e);
                std::process::exit(1);
            }
        }

        // wait
        let mut cmd = Cmd::new();
        cmd.arg("WAIT").arg(self.replica_count).arg(0);
        match cmd.query_async(&mut self.conn).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to execute `wait` command: {:?}", e);
                std::process::exit(1);
            }
        }
    }

    async fn info(&mut self, key: &str) -> InfoDict {
        match self.conn {
            ClientConnection::Standalone(ref mut conn) => redis::cmd("INFO").arg(key).query_async(conn).await.unwrap(),
            ClientConnection::Cluster(ref mut conn) => {
                let random = RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random);
                let value = conn.route_command(&mut redis::cmd("INFO").arg(key), random).await.unwrap();
                from_redis_value(&value).unwrap()
            }
        }
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

    /// return the `used_memory` in bytes
    pub async fn info_memory(&mut self) -> u64 {
        let (info_pkey, info_skey) = match &self.db_type {
            DBType::Redis => ("MEMORY", "used_memory"),
            DBType::Valkey => ("MEMORY", "used_memory"),
            DBType::Garnet => ("MEMORY", "proc_physical_memory_size"),
            DBType::TairMem => ("MEMORY", "used_memory"),
            DBType::TairScm => ("PERSISTENCE", "used_pmem"),
            DBType::TairSsd => ("PERSISTENCE", "data_used_disk_size"),
            DBType::Unknown(db) => {
                eprintln!("Unknown db type: {}", db);
                std::process::exit(1);
            }
        };
        let info: InfoDict = self.info(info_pkey).await;
        return match info.get(info_skey) {
            Some::<u64>(used_memory) => used_memory,
            None => {
                eprintln!("Failed to get `used_memory` from `info memory` command: {:?}", info);
                std::process::exit(1);
            }
        };
    }
}
