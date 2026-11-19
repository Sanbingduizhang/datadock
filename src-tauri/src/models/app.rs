//! IPC 数据模型。
//!
//! 所有对外结构统一使用 `#[serde(rename_all = "camelCase")]`，
//! 与前端 TypeScript 类型保持字段一致（见 `src/types/*.ts`）。

use serde::{Deserialize, Serialize};

/// 连通性检查请求
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    #[serde(default)]
    pub message: Option<String>,
}

/// 连通性检查响应
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    /// 后端回声内容
    pub reply: String,
    /// 后端处理时间（Unix 毫秒）
    pub received_at_ms: u64,
}

/// 应用与运行环境信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub log_dir: String,
    pub started_at_ms: u64,
}
