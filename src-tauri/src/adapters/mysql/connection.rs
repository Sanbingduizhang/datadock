//! MySQL 连接建立与基础探测。

use std::time::{Duration, Instant};

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use sqlx::MySqlPool;

use crate::errors::{error_code, AppError};
use crate::models::connection::{ConnectionTarget, DEFAULT_CHARSET};

pub struct TestOutcome {
    pub server_version: String,
    pub elapsed_ms: u64,
}

/// 把统一的 SslMode 映射到 sqlx 的 MySqlSslMode。
fn ssl_mode_from_ssl(mode: crate::models::connection::SslMode) -> MySqlSslMode {
    use crate::models::connection::SslMode;
    match mode {
        SslMode::Disabled => MySqlSslMode::Disabled,
        SslMode::Prefer => MySqlSslMode::Preferred,
        SslMode::Require => MySqlSslMode::Required,
        SslMode::VerifyCa => MySqlSslMode::VerifyCa,
        SslMode::VerifyFull => MySqlSslMode::VerifyIdentity,
    }
}

pub fn connect_options(target: &ConnectionTarget, password: &str) -> MySqlConnectOptions {
    let mut options = MySqlConnectOptions::new()
        .host(&target.host)
        .port(target.port)
        .username(&target.username)
        .password(password)
        .charset(target.charset.as_deref().unwrap_or(DEFAULT_CHARSET));

    if let Some(database) = target.default_database.as_deref() {
        options = options.database(database);
    }

    if target.options.ssl_enabled {
        let mode = target.options.ssl_mode.unwrap_or_default();
        options = options.ssl_mode(ssl_mode_from_ssl(mode));
    }

    options
}

/// 建立连接池。失败时统一转换为 CONNECTION_FAILED，消息中不含密码。
pub async fn create_pool(target: &ConnectionTarget, password: &str) -> Result<MySqlPool, AppError> {
    MySqlPoolOptions::new()
        .max_connections(4)
        .min_connections(0)
        .acquire_timeout(Duration::from_millis(target.connect_timeout_ms))
        .connect_with(connect_options(target, password))
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("连接 MySQL 失败：{error}"),
            )
        })
}

pub async fn server_version(pool: &MySqlPool) -> Result<String, AppError> {
    sqlx::query_scalar::<_, String>("SELECT VERSION()")
        .fetch_one(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("读取服务器版本失败：{error}"),
            )
        })
}

pub async fn server_connection_id(pool: &MySqlPool) -> Result<u64, AppError> {
    sqlx::query_scalar::<_, u64>("SELECT CONNECTION_ID()")
        .fetch_one(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("读取会话 ID 失败：{error}"),
            )
        })
}

/// 使用临时连接测试连通性，不保存任何配置。
pub async fn test(target: &ConnectionTarget, password: &str) -> Result<TestOutcome, AppError> {
    let started = Instant::now();
    let pool = create_pool(target, password).await?;
    let version = server_version(&pool).await;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    pool.close().await;

    Ok(TestOutcome {
        server_version: version?,
        elapsed_ms,
    })
}
