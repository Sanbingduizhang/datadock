//! SQL 查询历史模型（阶段 E 接入执行引擎时使用）。

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryHistoryStatus {
    Running,
    Success,
    Failed,
    Cancelled,
}

impl QueryHistoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryEntry {
    pub id: String,
    pub connection_id: Option<String>,
    pub source_type: String,
    pub database_name: Option<String>,
    pub sql_text: String,
    pub status: String,
    pub returned_rows: i64,
    pub affected_rows: Option<i64>,
    pub elapsed_ms: i64,
    pub error_message: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

/// 新建历史记录（开始时写入，状态为 running）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewQueryHistory {
    pub connection_id: Option<String>,
    pub source_type: String,
    pub database_name: Option<String>,
    pub sql_text: String,
}

/// 执行结束后的结果更新
#[derive(Debug, Clone)]
pub struct QueryHistoryOutcome {
    pub status: QueryHistoryStatus,
    pub returned_rows: i64,
    pub affected_rows: Option<i64>,
    pub elapsed_ms: i64,
    pub error_message: Option<String>,
    pub finished_at: Option<i64>,
}

/// 历史列表请求
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQueryHistoryRequest {
    pub connection_id: Option<String>,
    pub keyword: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl ListQueryHistoryRequest {
    /// 单页上限，避免一次拉取过多记录
    pub fn normalized_limit(&self) -> u32 {
        self.limit.unwrap_or(50).clamp(1, 500)
    }

    pub fn normalized_offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQueryHistoryResponse {
    pub items: Vec<QueryHistoryEntry>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteQueryHistoryRequest {
    pub history_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearQueryHistoryRequest {
    /// 不传表示清空全部历史
    pub connection_id: Option<String>,
}
