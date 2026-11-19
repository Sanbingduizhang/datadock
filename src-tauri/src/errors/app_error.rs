//! 应用统一错误类型。
//!
//! 所有 Command 都返回 `Result<T, AppError>`，序列化后的结构与前端
//! `src/types/api.ts` 中的 `AppError` 保持一致：
//!
//! ```json
//! { "code": "INVALID_ARGUMENT", "message": "可读错误信息", "details": { } }
//! ```

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use super::error_code;

/// 统一错误结构，可直接作为 Tauri Command 的错误返回。
#[derive(Debug, Clone, Serialize, Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// 程序可识别的错误码
    pub code: String,
    /// 面向用户的可读错误信息，不得包含密码等敏感内容
    pub message: String,
    /// 可选诊断信息，不得包含敏感数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// 参数校验失败
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(error_code::INVALID_ARGUMENT, message)
    }

    /// 本地存储失败
    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(error_code::STORAGE_ERROR, message)
    }

    /// 内部错误
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(error_code::INTERNAL_ERROR, message)
    }

    /// 系统凭据服务访问失败
    pub fn credential(message: impl Into<String>) -> Self {
        Self::new(error_code::CREDENTIAL_ACCESS_FAILED, message)
    }

    /// 附加诊断信息（调用方需确保不包含敏感数据）
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::storage(format!("本地文件操作失败：{error}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(format!("数据序列化失败：{error}"))
    }
}

impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        Self::internal(format!("Tauri 运行时错误：{error}"))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::storage(format!("本地数据库操作失败：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_frontend_contract() {
        let error =
            AppError::validation("参数无效").with_details(serde_json::json!({ "field": "host" }));
        let value = serde_json::to_value(&error).expect("错误结构应可序列化");

        assert_eq!(value["code"], "INVALID_ARGUMENT");
        assert_eq!(value["message"], "参数无效");
        assert_eq!(value["details"]["field"], "host");
    }

    #[test]
    fn omits_empty_details() {
        let value = serde_json::to_value(AppError::internal("boom")).expect("错误结构应可序列化");

        assert!(value.get("details").is_none());
        assert_eq!(value["code"], "INTERNAL_ERROR");
    }

    #[test]
    fn io_error_maps_to_storage_code() {
        let error: AppError =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied").into();

        assert_eq!(error.code, error_code::STORAGE_ERROR);
        assert!(error.message.contains("denied"));
    }
}
