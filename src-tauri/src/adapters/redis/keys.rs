//! Redis 键扫描与值读取。
//!
//! 使用 SCAN 游标迭代键（不阻塞主线程），批量获取类型与 TTL，
//! 按类型读取值内容（GET/HGETALL/LRANGE/SMEMBERS/ZRANGE/XRANGE）。

use redis::aio::MultiplexedConnection;
use redis::Value;

use crate::errors::{error_code, AppError};
use crate::models::redis::{
    HashField, ListItem, RedisKeyInfo, RedisKeyType, RedisKeyValue, RedisScanResponse, StreamEntry,
    ZsetEntry,
};

/// 扫描一批键（SCAN cursor MATCH pattern COUNT count）
pub async fn scan_keys(
    connection: &mut MultiplexedConnection,
    cursor: u64,
    pattern: &str,
    count: u32,
    include_system_keys: bool,
) -> Result<RedisScanResponse, AppError> {
    let (next_cursor, names): (u64, Vec<String>) = redis::cmd("SCAN")
        .arg(cursor)
        .arg("MATCH")
        .arg(pattern)
        .arg("COUNT")
        .arg(count)
        .query_async(connection)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("Redis SCAN 失败：{error}"),
            )
        })?;

    let mut keys: Vec<RedisKeyInfo> = Vec::with_capacity(names.len());
    for name in names {
        if !include_system_keys && name.starts_with('_') {
            continue;
        }

        // 获取类型与 TTL（键可能在扫描过程中被删除）
        let key_type: Result<String, _> =
            redis::cmd("TYPE").arg(&name).query_async(connection).await;

        let Ok(key_type) = key_type else {
            continue; // 键已删除或访问失败，跳过
        };

        let key_type = parse_key_type(&key_type);
        if key_type == RedisKeyType::None {
            continue; // 键已不存在
        }

        let ttl_seconds: i64 = redis::cmd("TTL")
            .arg(&name)
            .query_async(connection)
            .await
            .unwrap_or(-2);

        keys.push(RedisKeyInfo {
            name,
            key_type,
            ttl_seconds,
        });
    }

    Ok(RedisScanResponse {
        keys,
        cursor: next_cursor,
    })
}

/// 读取键值内容（按类型分发到对应命令）
pub async fn get_key_value(
    connection: &mut MultiplexedConnection,
    key_name: &str,
    max_items: u32,
) -> Result<(RedisKeyType, i64, RedisKeyValue), AppError> {
    // 获取类型
    let type_str: String = redis::cmd("TYPE")
        .arg(key_name)
        .query_async(connection)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("Redis TYPE 失败：{error}"),
            )
        })?;

    let key_type = parse_key_type(&type_str);
    if key_type == RedisKeyType::None {
        return Ok((RedisKeyType::None, -2, RedisKeyValue::None));
    }

    // 获取 TTL
    let ttl_seconds: i64 = redis::cmd("TTL")
        .arg(key_name)
        .query_async(connection)
        .await
        .unwrap_or(-2);

    // 按类型读取值
    let value = match key_type {
        RedisKeyType::String => {
            let value: Option<String> = redis::cmd("GET")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis GET 失败：{error}"),
                    )
                })?;
            RedisKeyValue::String { value }
        }
        RedisKeyType::Hash => {
            let pairs: Vec<(String, String)> = redis::cmd("HGETALL")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis HGETALL 失败：{error}"),
                    )
                })?;

            let fields: Vec<HashField> = pairs
                .into_iter()
                .take(max_items as usize)
                .map(|(field, value)| HashField { field, value })
                .collect();
            RedisKeyValue::Hash { fields }
        }
        RedisKeyType::List => {
            let items: Vec<String> = redis::cmd("LRANGE")
                .arg(key_name)
                .arg(0)
                .arg(max_items.saturating_sub(1))
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis LRANGE 失败：{error}"),
                    )
                })?;

            let items: Vec<ListItem> = items
                .into_iter()
                .enumerate()
                .map(|(index, value)| ListItem {
                    index: index as u64,
                    value,
                })
                .collect();
            RedisKeyValue::List { items }
        }
        RedisKeyType::Set => {
            let members: Vec<String> = redis::cmd("SMEMBERS")
                .arg(key_name)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis SMEMBERS 失败：{error}"),
                    )
                })?;

            let members: Vec<String> = members.into_iter().take(max_items as usize).collect();
            RedisKeyValue::Set { members }
        }
        RedisKeyType::Zset => {
            let entries: Vec<(String, f64)> = redis::cmd("ZRANGE")
                .arg(key_name)
                .arg(0)
                .arg(max_items.saturating_sub(1))
                .arg("WITHSCORES")
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis ZRANGE 失败：{error}"),
                    )
                })?;

            let entries: Vec<ZsetEntry> = entries
                .into_iter()
                .map(|(member, score)| ZsetEntry { member, score })
                .collect();
            RedisKeyValue::Zset { entries }
        }
        RedisKeyType::Stream => {
            let entries: Vec<(String, Vec<(String, String)>)> = redis::cmd("XRANGE")
                .arg(key_name)
                .arg("-")
                .arg("+")
                .arg("COUNT")
                .arg(max_items)
                .query_async(connection)
                .await
                .map_err(|error| {
                    AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("Redis XRANGE 失败：{error}"),
                    )
                })?;

            let entries: Vec<StreamEntry> = entries
                .into_iter()
                .map(|(id, fields)| StreamEntry {
                    id,
                    fields: fields
                        .into_iter()
                        .map(|(field, value)| HashField { field, value })
                        .collect(),
                })
                .collect();
            RedisKeyValue::Stream { entries }
        }
        RedisKeyType::None => RedisKeyValue::None,
    };

    Ok((key_type, ttl_seconds, value))
}

/// 解析 Redis TYPE 响应
fn parse_key_type(type_str: &str) -> RedisKeyType {
    match type_str.to_ascii_lowercase().as_str() {
        "string" => RedisKeyType::String,
        "hash" => RedisKeyType::Hash,
        "list" => RedisKeyType::List,
        "set" => RedisKeyType::Set,
        "zset" => RedisKeyType::Zset,
        "stream" => RedisKeyType::Stream,
        _ => RedisKeyType::None,
    }
}

/// 将 Value 格式化为字符串（用于 Stream 字段值）
#[allow(dead_code)]
fn format_value(value: &Value) -> String {
    match value {
        Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        Value::SimpleString(text) => text.clone(),
        Value::Int(number) => number.to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_types() {
        assert_eq!(parse_key_type("string"), RedisKeyType::String);
        assert_eq!(parse_key_type("HASH"), RedisKeyType::Hash);
        assert_eq!(parse_key_type("list"), RedisKeyType::List);
        assert_eq!(parse_key_type("set"), RedisKeyType::Set);
        assert_eq!(parse_key_type("zset"), RedisKeyType::Zset);
        assert_eq!(parse_key_type("stream"), RedisKeyType::Stream);
        assert_eq!(parse_key_type("none"), RedisKeyType::None);
        assert_eq!(parse_key_type("unknown"), RedisKeyType::None);
    }
}
