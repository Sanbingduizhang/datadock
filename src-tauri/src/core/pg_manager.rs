//! PostgreSQL 运行时连接管理。
//!
//! 与 MySQL 的 ConnectionManager 分开存放：
//! PG 使用独立单连接池语义，每个数据库一个池。

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::utils::time::now_ms;

pub struct ManagedPg {
    pub pool: PgPool,
    pub server_version: String,
    /// 连接参数（用于按 schema / 事务建立独立单连接池）
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: sqlx::postgres::PgSslMode,
    pub connected_at_ms: u64,
    /// 手动事务会话：每个事务一条独立单连接，提交/回滚后关闭
    transactions: RwLock<HashMap<String, PgPool>>,
}

impl ManagedPg {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: PgPool,
        server_version: String,
        host: String,
        port: u16,
        username: String,
        password: String,
        database: String,
        ssl_mode: sqlx::postgres::PgSslMode,
    ) -> Self {
        Self {
            pool,
            server_version,
            host,
            port,
            username,
            password,
            database,
            ssl_mode,
            connected_at_ms: now_ms(),
            transactions: RwLock::new(HashMap::new()),
        }
    }

    /// 当前数据库名
    pub fn current_database(&self) -> String {
        self.database.clone()
    }

    /// 登记事务会话（调用方已在此会话上执行 `BEGIN`）
    pub async fn register_transaction(&self, transaction_id: &str, pool: PgPool) {
        self.transactions
            .write()
            .await
            .insert(transaction_id.to_string(), pool);
    }

    /// 读取事务会话（执行语句时复用）
    pub async fn transaction_pool(&self, transaction_id: &str) -> Option<PgPool> {
        self.transactions.read().await.get(transaction_id).cloned()
    }

    /// 取出事务会话（调用方负责执行 COMMIT/ROLLBACK 并关闭）
    pub async fn take_transaction(&self, transaction_id: &str) -> Option<PgPool> {
        self.transactions.write().await.remove(transaction_id)
    }

    /// 关闭主池与所有事务会话（断开连接 / 应用退出时使用）
    pub async fn close_all(&self) {
        self.pool.close().await;

        let transactions: Vec<PgPool> = {
            let mut guard = self.transactions.write().await;
            guard.drain().map(|(_, pool)| pool).collect()
        };
        for pool in transactions {
            pool.close().await;
        }
    }
}

#[derive(Default)]
pub struct PgManager {
    connections: RwLock<HashMap<String, Arc<ManagedPg>>>,
}

impl PgManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, connection_id: String, connection: ManagedPg) {
        self.connections
            .write()
            .await
            .insert(connection_id, Arc::new(connection));
    }

    pub async fn get(&self, connection_id: &str) -> Option<Arc<ManagedPg>> {
        self.connections.read().await.get(connection_id).cloned()
    }

    pub async fn is_connected(&self, connection_id: &str) -> bool {
        self.connections.read().await.contains_key(connection_id)
    }

    pub async fn remove(&self, connection_id: &str) -> Option<Arc<ManagedPg>> {
        self.connections.write().await.remove(connection_id)
    }

    pub async fn drain(&self) -> Vec<(String, Arc<ManagedPg>)> {
        let mut guard = self.connections.write().await;
        guard.drain().collect()
    }
}
