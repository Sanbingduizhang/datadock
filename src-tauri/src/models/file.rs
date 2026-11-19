//! SQL 文件读写模型。

use serde::{Deserialize, Serialize};

/// 单个 SQL 文件的读取上限（5 MiB），避免误开超大文件卡死界面
pub const MAX_SQL_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSqlFileRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSqlFileResponse {
    pub path: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSqlFileRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteSqlFileResponse {
    pub path: String,
    pub name: String,
    pub bytes: u64,
}
