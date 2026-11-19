//! Redis 运行时连接管理。
//!
//! 与 MySQL 的 `ConnectionManager` 分开存放：Redis 没有连接池语义，
//! 使用 redis crate 的多路复用连接即可并发执行命令。
//!
//! `ManagedRedis` 额外保存连接端点信息，以便按库索引（db0..dbN）建立
//! 独立的多路复用连接，从而支持「切换数据库浏览」。

use std::collections::HashMap;
use std::sync::Arc;

use redis::aio::MultiplexedConnection;
use redis::{Client, ConnectionAddr, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};
use tokio::sync::RwLock;

use crate::errors::{error_code, AppError};
use crate::utils::time::now_ms;

/// Redis 连接端点（用于按库重建连接）。
#[derive(Debug, Clone)]
pub struct RedisEndpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// 是否走 TLS 加密连接
    pub use_tls: bool,
}

impl RedisEndpoint {
    fn connection_info(&self, db: u32) -> ConnectionInfo {
        let addr = if self.use_tls {
            ConnectionAddr::TcpTls {
                host: self.host.clone(),
                port: self.port,
                insecure: false,
                tls_params: None,
            }
        } else {
            ConnectionAddr::Tcp(self.host.clone(), self.port)
        };

        ConnectionInfo {
            addr,
            redis: RedisConnectionInfo {
                db: i64::from(db),
                username: (!self.username.is_empty()).then(|| self.username.clone()),
                password: (!self.password.is_empty()).then(|| self.password.clone()),
                protocol: ProtocolVersion::RESP2,
            },
        }
    }

    /// 建立指向指定库（db0..dbN）的独立多路复用连接。
    pub async fn connect_db(&self, db: u32) -> Result<MultiplexedConnection, AppError> {
        let client = Client::open(self.connection_info(db)).map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("Redis 连接参数无效：{error}"),
            )
        })?;

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                "连接 Redis 超时，请检查主机与端口",
            )
        })?
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("连接 Redis 失败：{error}"),
            )
        })
    }
}

pub struct ManagedRedis {
    pub connection: MultiplexedConnection,
    pub server_version: String,
    /// 当前选中的 Redis 库索引（0-15）
    pub database: u32,
    /// 连接端点（用于按库重建连接）
    pub endpoint: RedisEndpoint,
    pub connected_at_ms: u64,
}

impl ManagedRedis {
    pub fn new(
        connection: MultiplexedConnection,
        server_version: String,
        database: u32,
        endpoint: RedisEndpoint,
    ) -> Self {
        Self {
            connection,
            server_version,
            database,
            endpoint,
            connected_at_ms: now_ms(),
        }
    }

    pub fn current_database(&self) -> String {
        format!("db{}", self.database)
    }

    /// 建立指向指定库的独立连接（用于切换库浏览）。
    pub async fn connect_db(&self, db: u32) -> Result<MultiplexedConnection, AppError> {
        self.endpoint.connect_db(db).await
    }
}

#[derive(Default)]
pub struct RedisManager {
    connections: RwLock<HashMap<String, Arc<ManagedRedis>>>,
}

impl RedisManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, connection_id: String, connection: ManagedRedis) {
        self.connections
            .write()
            .await
            .insert(connection_id, Arc::new(connection));
    }

    pub async fn get(&self, connection_id: &str) -> Option<Arc<ManagedRedis>> {
        self.connections.read().await.get(connection_id).cloned()
    }

    pub async fn is_connected(&self, connection_id: &str) -> bool {
        self.connections.read().await.contains_key(connection_id)
    }

    pub async fn remove(&self, connection_id: &str) -> Option<Arc<ManagedRedis>> {
        self.connections.write().await.remove(connection_id)
    }

    pub async fn len(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.connections.read().await.is_empty()
    }

    pub async fn drain(&self) -> Vec<(String, Arc<ManagedRedis>)> {
        let mut guard = self.connections.write().await;
        guard.drain().collect()
    }
}
