//! 应用级基础服务：运行环境信息与连通性检查。

use std::path::PathBuf;

use crate::errors::AppError;
use crate::models::app::{AppInfo, PingRequest, PingResponse};

/// ping 消息的最大长度，避免无意义的大数据传输
const MAX_PING_MESSAGE_LEN: usize = 500;

/// 应用基础服务，由 `AppState` 以 `Arc` 共享给各 Command。
pub struct AppService {
    data_dir: PathBuf,
    log_dir: PathBuf,
    started_at_ms: u64,
}

impl AppService {
    pub fn new(data_dir: PathBuf, log_dir: PathBuf) -> Self {
        Self {
            data_dir,
            log_dir,
            started_at_ms: now_ms(),
        }
    }

    /// IPC 连通性检查
    pub fn ping(&self, request: PingRequest) -> Result<PingResponse, AppError> {
        let reply = match request.message.as_deref().map(str::trim) {
            None | Some("") => "pong".to_string(),
            Some(message) => {
                if message.chars().count() > MAX_PING_MESSAGE_LEN {
                    return Err(AppError::validation(format!(
                        "消息长度超过限制，最多 {MAX_PING_MESSAGE_LEN} 个字符"
                    ))
                    .with_details(serde_json::json!({ "field": "message", "maxLength": MAX_PING_MESSAGE_LEN })));
                }
                format!("pong: {message}")
            }
        };

        Ok(PingResponse {
            reply,
            received_at_ms: now_ms(),
        })
    }

    /// 汇总应用与运行环境信息
    pub fn app_info(&self, name: &str, version: &str) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            version: version.to_string(),
            tauri_version: tauri::VERSION.to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            data_dir: self.data_dir.to_string_lossy().into_owned(),
            log_dir: self.log_dir.to_string_lossy().into_owned(),
            started_at_ms: self.started_at_ms,
        }
    }
}

/// 当前 Unix 毫秒时间戳（兼容旧的调用路径，实际实现位于 utils::time）
pub use crate::utils::time::now_ms;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::error_code;

    fn service() -> AppService {
        AppService::new(PathBuf::from("/tmp/data"), PathBuf::from("/tmp/logs"))
    }

    #[test]
    fn ping_without_message_returns_pong() {
        let response = service().ping(PingRequest::default()).expect("ping 应成功");

        assert_eq!(response.reply, "pong");
        assert!(response.received_at_ms > 0);
    }

    #[test]
    fn ping_echoes_trimmed_message() {
        let request = PingRequest {
            message: Some("  hello  ".to_string()),
        };
        let response = service().ping(request).expect("ping 应成功");

        assert_eq!(response.reply, "pong: hello");
    }

    #[test]
    fn ping_rejects_oversized_message() {
        let request = PingRequest {
            message: Some("x".repeat(MAX_PING_MESSAGE_LEN + 1)),
        };
        let error = service().ping(request).expect_err("超长消息应被拒绝");

        assert_eq!(error.code, error_code::INVALID_ARGUMENT);
    }

    #[test]
    fn app_info_reports_directories() {
        let info = service().app_info("DataDock", "0.1.0");

        assert_eq!(info.name, "DataDock");
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.data_dir, "/tmp/data");
        assert_eq!(info.log_dir, "/tmp/logs");
    }
}
