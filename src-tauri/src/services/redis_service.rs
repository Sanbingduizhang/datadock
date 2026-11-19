//! Redis 业务：命令执行、资源信息与键浏览。

use std::sync::Arc;

use redis::aio::MultiplexedConnection;

use crate::adapters::redis::command as redis_command;
use crate::adapters::redis::edit as redis_edit;
use crate::adapters::redis::keys as redis_keys;
use crate::core::{ManagedRedis, RedisManager};
use crate::errors::{error_code, AppError};
use crate::models::connection::SimpleSuccessResponse;
use crate::models::database::DatabaseInfo;
use crate::models::query::ExecuteSqlResponse;
use crate::models::redis::{
    RedisDeleteKeyRequest, RedisKeyValueRequest, RedisKeyValueResponse, RedisRenameKeyRequest,
    RedisScanRequest, RedisScanResponse, RedisSetKeyValueRequest, RedisSetTtlRequest,
};

/// Redis 库数量上限（配置为 16 个库时显示 db0..db15）。
const REDIS_MAX_DB: u32 = 16;

pub struct RedisService {
    manager: Arc<RedisManager>,
}

impl RedisService {
    pub fn new(manager: Arc<RedisManager>) -> Self {
        Self { manager }
    }

    pub async fn execute(
        &self,
        connection_id: &str,
        command: &str,
    ) -> Result<ExecuteSqlResponse, AppError> {
        let managed = self.manager.get(connection_id).await.ok_or_else(|| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                "Redis 连接尚未建立，请先连接",
            )
        })?;

        let mut connection = managed.connection.clone();
        redis_command::execute(&mut connection, command).await
    }

    /// 返回 Redis 可浏览的库列表。
    ///
    /// 用户配置了默认库时优先返回它；否则依次探测 db0..db15，只返回
    /// 「有数据」的库（DBSIZE > 0）。若所有库都为空，则回退到 db0，
    /// 保证资源树至少有一个可展开的节点。
    pub async fn list_databases(&self, connection_id: &str) -> Result<Vec<DatabaseInfo>, AppError> {
        let managed = self.manager.get(connection_id).await.ok_or_else(|| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                "Redis 连接尚未建立，请先连接",
            )
        })?;

        // 若连接时显式指定了库（默认库 > 0），直接返回该库，尊重用户配置
        if managed.database > 0 {
            return Ok(vec![DatabaseInfo {
                name: managed.current_database(),
            }]);
        }

        // 否则探测各库是否有数据，返回有数据的库；全部为空则回退 db0
        let mut non_empty: Vec<DatabaseInfo> = Vec::new();
        for db in 0..REDIS_MAX_DB {
            let mut connection = match managed.endpoint.connect_db(db).await {
                Ok(connection) => connection,
                Err(_) => continue, // 某些库不可用，跳过
            };
            let size: i64 = match redis::cmd("DBSIZE").query_async(&mut connection).await {
                Ok(size) => size,
                Err(_) => continue,
            };
            if size > 0 {
                non_empty.push(DatabaseInfo {
                    name: format!("db{db}"),
                });
            }
        }

        if non_empty.is_empty() {
            Ok(vec![DatabaseInfo {
                name: "db0".to_string(),
            }])
        } else {
            Ok(non_empty)
        }
    }

    /// 扫描 Redis 键（SCAN 游标迭代）。
    ///
    /// 若请求里显式带了 `database`（如 `db3`），会在该库上建立独立连接
    /// 进行扫描，实现「切换库浏览」。
    pub async fn scan_keys(
        &self,
        request: RedisScanRequest,
    ) -> Result<RedisScanResponse, AppError> {
        let managed = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "Redis 连接尚未建立，请先连接",
                )
            })?;

        let pattern = request.pattern.as_deref().unwrap_or("*");
        let count = request.count.unwrap_or(100);
        let include_system_keys = request.include_system_keys;

        // 目标库：请求里带了库则切过去，否则用当前连接的库
        let db = request.database.unwrap_or(managed.database);
        let mut connection = if db == managed.database {
            managed.connection.clone()
        } else {
            managed.endpoint.connect_db(db).await?
        };

        redis_keys::scan_keys(
            &mut connection,
            0, // 起始游标
            pattern,
            count,
            include_system_keys,
        )
        .await
    }

    /// 读取 Redis 键值内容
    pub async fn get_key_value(
        &self,
        request: RedisKeyValueRequest,
    ) -> Result<RedisKeyValueResponse, AppError> {
        let managed = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "Redis 连接尚未建立，请先连接",
                )
            })?;

        let max_items = request.max_items.unwrap_or(100);

        // 目标库：请求里带了库则切过去，否则用当前连接的库
        let db = request.database.unwrap_or(managed.database);
        let mut connection = if db == managed.database {
            managed.connection.clone()
        } else {
            managed.endpoint.connect_db(db).await?
        };

        let (key_type, ttl_seconds, value) =
            redis_keys::get_key_value(&mut connection, &request.key_name, max_items).await?;

        Ok(RedisKeyValueResponse {
            key_name: request.key_name,
            key_type,
            ttl_seconds,
            value,
        })
    }

    // 取得指向目标库的连接
    async fn connection_for(
        &self,
        connection_id: &str,
        database: Option<u32>,
    ) -> Result<(Arc<ManagedRedis>, MultiplexedConnection), AppError> {
        let managed = self.manager.get(connection_id).await.ok_or_else(|| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                "Redis 连接尚未建立，请先连接",
            )
        })?;

        let db = database.unwrap_or(managed.database);
        let connection = if db == managed.database {
            managed.connection.clone()
        } else {
            managed.endpoint.connect_db(db).await?
        };

        Ok((managed, connection))
    }

    /// 删除键
    pub async fn delete_key(
        &self,
        request: RedisDeleteKeyRequest,
    ) -> Result<SimpleSuccessResponse, AppError> {
        let (_, mut connection) = self
            .connection_for(&request.connection_id, request.database)
            .await?;

        redis_edit::delete_key(&mut connection, &request.key_name).await?;
        Ok(SimpleSuccessResponse { success: true })
    }

    /// 重命名键
    pub async fn rename_key(
        &self,
        request: RedisRenameKeyRequest,
    ) -> Result<SimpleSuccessResponse, AppError> {
        let (_, mut connection) = self
            .connection_for(&request.connection_id, request.database)
            .await?;

        redis_edit::rename_key(&mut connection, &request.from, &request.to).await?;
        Ok(SimpleSuccessResponse { success: true })
    }

    /// 设置过期时间
    pub async fn set_ttl(
        &self,
        request: RedisSetTtlRequest,
    ) -> Result<SimpleSuccessResponse, AppError> {
        let (_, mut connection) = self
            .connection_for(&request.connection_id, request.database)
            .await?;

        redis_edit::set_ttl(&mut connection, &request.key_name, request.ttl_seconds).await?;
        Ok(SimpleSuccessResponse { success: true })
    }

    /// 写入键值（编辑或新增）
    pub async fn set_key_value(
        &self,
        request: RedisSetKeyValueRequest,
    ) -> Result<SimpleSuccessResponse, AppError> {
        let (_, mut connection) = self
            .connection_for(&request.connection_id, request.database)
            .await?;

        redis_edit::set_key_value(&mut connection, &request.key_name, &request.value).await?;

        // 若请求带 TTL，写入后设置过期
        if let Some(ttl) = request.ttl_seconds {
            redis_edit::set_ttl(&mut connection, &request.key_name, ttl).await?;
        }

        Ok(SimpleSuccessResponse { success: true })
    }
}
