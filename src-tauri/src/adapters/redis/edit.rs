//! Redis 键写操作：删除、重命名、TTL、值编辑与新增。
//!
//! 删除/重命名/TTL 对所有类型通用；值编辑按类型分发到对应命令
//! （SET / HSET / RPUSH / SADD / ZADD / XADD）。

use redis::aio::MultiplexedConnection;

use crate::errors::{error_code, AppError};
use crate::models::redis::RedisKeyValue;

/// 删除键（DEL）。返回是否实际删除（键不存在返回 false）。
pub async fn delete_key(
    connection: &mut MultiplexedConnection,
    key_name: &str,
) -> Result<bool, AppError> {
    let deleted: i64 = redis::cmd("DEL")
        .arg(key_name)
        .query_async(connection)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("Redis DEL 失败：{error}"),
            )
        })?;

    Ok(deleted > 0)
}

/// 重命名键（RENAME）。目标已存在时会被覆盖。
pub async fn rename_key(
    connection: &mut MultiplexedConnection,
    from: &str,
    to: &str,
) -> Result<(), AppError> {
    let _: String = redis::cmd("RENAME")
        .arg(from)
        .arg(to)
        .query_async(connection)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("Redis RENAME 失败：{error}"),
            )
        })?;

    Ok(())
}

/// 设置过期时间（EXPIRE，秒）。`ttl_seconds <= 0` 时移除过期（PERSIST）。
pub async fn set_ttl(
    connection: &mut MultiplexedConnection,
    key_name: &str,
    ttl_seconds: i64,
) -> Result<(), AppError> {
    let result: Result<String, _> = if ttl_seconds > 0 {
        redis::cmd("EXPIRE")
            .arg(key_name)
            .arg(ttl_seconds)
            .query_async(connection)
            .await
            .map(|_: i64| "OK".to_string())
    } else {
        redis::cmd("PERSIST")
            .arg(key_name)
            .query_async(connection)
            .await
            .map(|_: i64| "OK".to_string())
    };

    result.map_err(|error| {
        AppError::new(
            error_code::SQL_EXECUTION_FAILED,
            format!("Redis 设置过期时间失败：{error}"),
        )
    })?;

    Ok(())
}

/// 写入键值（按类型分发）。用于编辑已有键或新增键。
pub async fn set_key_value(
    connection: &mut MultiplexedConnection,
    key_name: &str,
    value: &RedisKeyValue,
) -> Result<(), AppError> {
    match value {
        RedisKeyValue::String { value } => {
            // 新增 string 用 SET；值为 None 时视作删除
            let _: String = redis::cmd("SET")
                .arg(key_name)
                .arg(value.clone().unwrap_or_default())
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis SET 失败：{error}"),
                    )
                })?;
        }
        RedisKeyValue::Hash { fields } => {
            // 覆盖式写入：先删再批量 HSET
            let _: i64 = redis::cmd("DEL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis DEL 失败：{error}"),
                    )
                })?;
            for field in fields {
                let _: i64 = redis::cmd("HSET")
                    .arg(key_name)
                    .arg(&field.field)
                    .arg(&field.value)
                    .query_async(connection)
                    .await
                    .map_err(|error| {
                        AppError::new(
                            error_code::SQL_EXECUTION_FAILED,
                            format!("Redis HSET 失败：{error}"),
                        )
                    })?;
            }
        }
        RedisKeyValue::List { items } => {
            let _: i64 = redis::cmd("DEL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis DEL 失败：{error}"),
                    )
                })?;
            if !items.is_empty() {
                let mut command = redis::cmd("RPUSH");
                command.arg(key_name);
                for item in items {
                    command.arg(&item.value);
                }
                let _: i64 = command.query_async(connection).await.map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis RPUSH 失败：{error}"),
                    )
                })?;
            }
        }
        RedisKeyValue::Set { members } => {
            let _: i64 = redis::cmd("DEL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis DEL 失败：{error}"),
                    )
                })?;
            if !members.is_empty() {
                let mut command = redis::cmd("SADD");
                command.arg(key_name);
                for member in members {
                    command.arg(member);
                }
                let _: i64 = command.query_async(connection).await.map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis SADD 失败：{error}"),
                    )
                })?;
            }
        }
        RedisKeyValue::Zset { entries } => {
            let _: i64 = redis::cmd("DEL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis DEL 失败：{error}"),
                    )
                })?;
            for entry in entries {
                let _: i64 = redis::cmd("ZADD")
                    .arg(key_name)
                    .arg(entry.score)
                    .arg(&entry.member)
                    .query_async(connection)
                    .await
                    .map_err(|error| {
                        AppError::new(
                            error_code::SQL_EXECUTION_FAILED,
                            format!("Redis ZADD 失败：{error}"),
                        )
                    })?;
            }
        }
        RedisKeyValue::Stream { entries } => {
            let _: i64 = redis::cmd("DEL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis DEL 失败：{error}"),
                    )
                })?;
            for entry in entries {
                let mut command = redis::cmd("XADD");
                command.arg(key_name);
                // 显式 ID；"*" 表示自动生成
                command.arg(&entry.id);
                for field in &entry.fields {
                    command.arg(&field.field).arg(&field.value);
                }
                let _: String = command.query_async(connection).await.map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis XADD 失败：{error}"),
                    )
                })?;
            }
        }
        RedisKeyValue::None => {
            return Err(AppError::new(
                error_code::INVALID_ARGUMENT,
                "不支持的键值类型，无法写入",
            ));
        }
    }

    Ok(())
}
