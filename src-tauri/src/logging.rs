//! 日志初始化。
//!
//! 日志同时输出到标准输出与本地日志目录（按天滚动）。
//! 严禁记录数据库密码、完整连接串或其他敏感数据。

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::errors::AppError;

/// 持有非阻塞写入线程的守护句柄，必须在进程存活期间保持有效，
/// 否则退出时可能丢失日志。
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// 初始化日志系统，返回日志目录。
pub fn init(app: &AppHandle) -> Result<PathBuf, AppError> {
    let log_dir = app.path().app_log_dir()?;
    fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "datadock.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // 重复初始化时保留第一个守护句柄，避免日志被提前刷出
    let _ = LOG_GUARD.set(guard);

    // 默认：全局 info，本项目模块 debug；RUST_LOG 可覆盖
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,datadock_lib=debug,datadock=debug"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_ansi(cfg!(debug_assertions));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_ansi(false)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|error| AppError::internal(format!("日志系统初始化失败：{error}")))?;

    Ok(log_dir)
}
