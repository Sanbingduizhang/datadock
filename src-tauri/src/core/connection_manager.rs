//! 运行时连接管理。
//!
//! 每个连接拥有独立连接池与运行时信息，互不共享；
//! 这里的对象只存在于内存，密码始终从系统凭据读取。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::MySqlPool;
use tokio::sync::RwLock;

use crate::errors::{error_code, AppError};
use crate::utils::time::now_ms;

/// 单个已建立的运行时连接
pub struct ManagedConnection {
    /// 默认库连接池
    pub pool: MySqlPool,
    /// 连接参数（含密码，仅存在于内存，用于按库建立独立连接池）
    options: MySqlConnectOptions,
    /// 按数据库缓存的连接池，保证不同库上下文的查询彼此隔离
    database_pools: RwLock<HashMap<String, MySqlPool>>,
    /// 手动事务会话：每个事务一条独立连接，提交/回滚后关闭
    transactions: RwLock<HashMap<String, MySqlPool>>,
    pub server_version: String,
    /// 服务端会话 ID，用于 `KILL QUERY` 取消（阶段 E）
    pub server_connection_id: u64,
    pub connected_at_ms: u64,
    pub current_database: Option<String>,
}

impl ManagedConnection {
    /// 生成带指定默认库的连接参数（含密码，只在内存中使用）。
    ///
    /// 查询执行使用独立的单连接会话，便于精确控制 `SET SESSION` 与 `KILL QUERY`。
    pub fn query_options(&self, database: Option<&str>) -> MySqlConnectOptions {
        match database {
            Some(database) => self.options.clone().database(database),
            None => self.options.clone(),
        }
    }

    /// 获取指定数据库的连接池；未指定或与默认库一致时复用主池。
    pub async fn pool_for(&self, database: Option<&str>) -> Result<MySqlPool, AppError> {
        let Some(database) = database else {
            return Ok(self.pool.clone());
        };

        if Some(database) == self.current_database.as_deref() {
            return Ok(self.pool.clone());
        }

        if let Some(pool) = self.database_pools.read().await.get(database) {
            return Ok(pool.clone());
        }

        let options = self.options.clone().database(database);
        let pool = MySqlPoolOptions::new()
            .max_connections(2)
            .idle_timeout(Duration::from_secs(120))
            .connect_with(options)
            .await
            .map_err(|error| {
                AppError::new(
                    error_code::DATABASE_NOT_FOUND,
                    format!("连接数据库 {database} 失败：{error}"),
                )
            })?;

        self.database_pools
            .write()
            .await
            .insert(database.to_string(), pool.clone());

        Ok(pool)
    }

    pub async fn register_transaction(&self, transaction_id: &str, pool: MySqlPool) {
        self.transactions
            .write()
            .await
            .insert(transaction_id.to_string(), pool);
    }

    pub async fn transaction_pool(&self, transaction_id: &str) -> Option<MySqlPool> {
        self.transactions.read().await.get(transaction_id).cloned()
    }

    /// 取出事务连接池（调用方负责执行 COMMIT/ROLLBACK 并关闭）
    pub async fn take_transaction(&self, transaction_id: &str) -> Option<MySqlPool> {
        self.transactions.write().await.remove(transaction_id)
    }

    /// 关闭主池与所有按库连接池
    pub async fn close_all(&self) {
        self.pool.close().await;

        let transactions: Vec<MySqlPool> = {
            let mut guard = self.transactions.write().await;
            guard.drain().map(|(_, pool)| pool).collect()
        };
        for pool in transactions {
            pool.close().await;
        }
        let pools: Vec<MySqlPool> = {
            let mut guard = self.database_pools.write().await;
            guard.drain().map(|(_, pool)| pool).collect()
        };
        for pool in pools {
            pool.close().await;
        }
    }

    pub fn new(
        pool: MySqlPool,
        options: MySqlConnectOptions,
        server_version: String,
        server_connection_id: u64,
        current_database: Option<String>,
    ) -> Self {
        Self {
            pool,
            options,
            database_pools: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
            server_version,
            server_connection_id,
            connected_at_ms: now_ms(),
            current_database,
        }
    }
}

#[derive(Default)]
pub struct ConnectionManager {
    connections: RwLock<HashMap<String, Arc<ManagedConnection>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, connection_id: String, connection: ManagedConnection) {
        self.connections
            .write()
            .await
            .insert(connection_id, Arc::new(connection));
    }

    pub async fn get(&self, connection_id: &str) -> Option<Arc<ManagedConnection>> {
        self.connections.read().await.get(connection_id).cloned()
    }

    pub async fn is_connected(&self, connection_id: &str) -> bool {
        self.connections.read().await.contains_key(connection_id)
    }

    /// 移除并返回运行时连接，调用方负责关闭连接池。
    pub async fn remove(&self, connection_id: &str) -> Option<Arc<ManagedConnection>> {
        self.connections.write().await.remove(connection_id)
    }

    pub async fn len(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.connections.read().await.is_empty()
    }

    /// 清空全部运行时连接（应用退出时使用）
    pub async fn drain(&self) -> Vec<(String, Arc<ManagedConnection>)> {
        let mut guard = self.connections.write().await;
        let drained = guard.drain().collect();
        drained
    }
}

/// 关闭运行时连接池（含按库连接池）
pub async fn close_connection(connection: Arc<ManagedConnection>) {
    connection.close_all().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_tracks_per_connection_entries() {
        // 连接池无法在单元测试中伪造，这里只验证空状态与移除语义
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("运行时应可创建");

        runtime.block_on(async {
            let manager = ConnectionManager::new();
            assert_eq!(manager.len().await, 0);
            assert!(manager.is_empty().await);
            assert!(manager.get("missing").await.is_none());
            assert!(manager.remove("missing").await.is_none());
            assert!(manager.drain().await.is_empty());
        });
    }
}
