//! Redis 连接建立与版本探测。

use std::time::Duration;

use redis::aio::MultiplexedConnection;
use redis::{Client, ConnectionAddr, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

use crate::errors::{error_code, AppError};

pub struct RedisParams<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
    /// Redis 的库索引（0-15）
    pub database: u32,
    /// 是否走 TLS 加密连接
    pub use_tls: bool,
}

pub async fn connect(
    params: &RedisParams<'_>,
) -> Result<(MultiplexedConnection, String), AppError> {
    let addr = if params.use_tls {
        ConnectionAddr::TcpTls {
            host: params.host.to_string(),
            port: params.port,
            insecure: false,
            tls_params: None,
        }
    } else {
        ConnectionAddr::Tcp(params.host.to_string(), params.port)
    };

    let info = ConnectionInfo {
        addr,
        redis: RedisConnectionInfo {
            db: i64::from(params.database),
            username: (!params.username.is_empty()).then(|| params.username.to_string()),
            password: (!params.password.is_empty()).then(|| params.password.to_string()),
            protocol: ProtocolVersion::RESP2,
        },
    };

    let client = Client::open(info).map_err(|error| {
        AppError::new(
            error_code::CONNECTION_FAILED,
            format!("Redis 连接参数无效：{error}"),
        )
    })?;

    let mut connection = tokio::time::timeout(
        Duration::from_secs(10),
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
    })?;

    let version = server_version(&mut connection).await;
    Ok((connection, version))
}

async fn server_version(connection: &mut MultiplexedConnection) -> String {
    let info: Result<String, _> = redis::cmd("INFO")
        .arg("server")
        .query_async(connection)
        .await;

    info.map(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("redis_version:"))
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    })
    .unwrap_or_else(|_| "unknown".to_string())
}

/// 临时连接用于测试连通性
pub async fn test(params: &RedisParams<'_>) -> Result<String, AppError> {
    let (mut connection, version) = connect(params).await?;
    let pong: String = redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("Redis PING 失败：{error}"),
            )
        })?;

    if !pong.eq_ignore_ascii_case("pong") {
        return Err(AppError::new(
            error_code::CONNECTION_FAILED,
            format!("Redis 返回了非预期响应：{pong}"),
        ));
    }

    Ok(version)
}
