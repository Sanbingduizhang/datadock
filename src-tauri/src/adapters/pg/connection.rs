//! PostgreSQL 连接参数与连接池创建。

use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

use crate::errors::{error_code, AppError};

pub struct PgParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    pub ssl_mode: PgSslMode,
    pub connect_timeout: Duration,
}

pub fn build_options(params: &PgParams) -> PgConnectOptions {
    let mut options = PgConnectOptions::new()
        .host(&params.host)
        .port(params.port)
        .username(&params.username)
        .ssl_mode(params.ssl_mode);

    if !params.password.is_empty() {
        options = options.password(&params.password);
    }
    if let Some(database) = params.database.as_deref() {
        if !database.trim().is_empty() {
            options = options.database(database.trim());
        }
    }
    options
}

/// 建立独立单连接池（每次执行/查询单独会话时使用）
pub async fn connect_single(params: &PgParams) -> Result<PgPool, AppError> {
    let options = build_options(params);
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(params.connect_timeout)
        .connect_with(options)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("连接 PostgreSQL 失败：{error}"),
            )
        })
}

/// 建立多连接池（长连接场景）
pub async fn connect_pool(params: &PgParams, max_connections: u32) -> Result<PgPool, AppError> {
    let options = build_options(params);
    PgPoolOptions::new()
        .max_connections(max_connections.max(1))
        .acquire_timeout(params.connect_timeout)
        .connect_with(options)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_FAILED,
                format!("连接 PostgreSQL 失败：{error}"),
            )
        })
}

pub fn ssl_mode_from_str(value: &str) -> PgSslMode {
    PgSslMode::from_str(value).unwrap_or(PgSslMode::Prefer)
}

/// 把统一的 SslMode 映射到 sqlx 的 PgSslMode。
pub fn ssl_mode_from_ssl(mode: crate::models::connection::SslMode) -> PgSslMode {
    use crate::models::connection::SslMode;
    match mode {
        SslMode::Disabled => PgSslMode::Disable,
        SslMode::Prefer => PgSslMode::Prefer,
        SslMode::Require => PgSslMode::Require,
        SslMode::VerifyCa => PgSslMode::VerifyCa,
        SslMode::VerifyFull => PgSslMode::VerifyFull,
    }
}
