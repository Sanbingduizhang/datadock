//! SQL 执行结果模型。
//!
//! 值统一以「类型标签 + 文本值」表示：整数与 DECIMAL 用字符串承载，
//! 避免经过 f64 造成精度损失（见 TDR-001）。
//!
//! 多语句执行按顺序进行，每条语句产生一个 `StatementResult`；
//! 遇到错误立即停止，错误通过 `error` 字段返回，已完成的结果仍然保留。

use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_ROWS: u32 = 1_000;
pub const MAX_ROWS_LIMIT: u32 = 10_000;

/// 单次执行的语句数量上限，防止误粘贴整份脚本导致界面卡死
pub const MAX_STATEMENTS: usize = 200;

/// 默认查询历史保留条数（对应 `query.history_limit` 默认值）
pub const DEFAULT_HISTORY_LIMIT: u32 = 1_000;

/// 语句预览长度，用于结果标签与历史列表
pub const SQL_PREVIEW_LIMIT: usize = 120;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumn {
    pub name: String,
    /// 驱动报告的字段类型，例如 `VARCHAR`、`BIGINT UNSIGNED`
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseValue {
    /// null / integer / decimal / float / boolean / string / datetime / json / binary
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl DatabaseValue {
    pub fn null() -> Self {
        Self {
            value_type: "null".to_string(),
            value: None,
        }
    }

    pub fn text(value_type: &str, value: impl Into<String>) -> Self {
        Self {
            value_type: value_type.to_string(),
            value: Some(value.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultSet {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<DatabaseValue>>,
    pub row_count: usize,
    /// 是否因为行数上限被截断
    pub truncated: bool,
}

/// 单条语句的执行结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    /// 语句序号，从 1 开始
    pub index: u32,
    /// 语句预览，用于结果标签
    pub sql_preview: String,
    /// 返回结果集的语句（SELECT/SHOW/...）才有值
    pub result_set: Option<QueryResultSet>,
    pub affected_rows: Option<u64>,
    pub last_insert_id: Option<String>,
    pub elapsed_ms: u64,
    pub message: String,
}

/// 语句执行失败信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementError {
    pub index: u32,
    pub sql_preview: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteSqlRequest {
    pub connection_id: String,
    pub database_name: Option<String>,
    pub sql: String,
    pub max_rows: Option<u32>,
    /// 前端生成的执行 ID，用于取消；缺省时后端自行生成
    pub task_id: Option<String>,
    /// 来源类型，写入查询历史（query / table / explain）
    pub source_type: Option<String>,
    /// 在指定事务会话中执行（提交/回滚前有效）
    pub transaction_id: Option<String>,
}

impl ExecuteSqlRequest {
    pub fn normalized_max_rows(&self) -> u32 {
        self.max_rows
            .unwrap_or(DEFAULT_MAX_ROWS)
            .clamp(1, MAX_ROWS_LIMIT)
    }

    pub fn source_type(&self) -> &str {
        self.source_type.as_deref().unwrap_or("query")
    }
}

/// 批量执行（用于表数据的多行变更）：整体包在一个事务里，失败自动回滚
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteBatchResponse {
    pub results: Vec<StatementResult>,
    /// 失败语句信息；为 null 表示全部成功并已提交
    pub error: Option<StatementError>,
    /// 是否因失败而回滚
    pub rolled_back: bool,
    pub elapsed_ms: u64,
    pub message: String,
    pub history_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginTransactionRequest {
    pub connection_id: String,
    pub database_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginTransactionResponse {
    pub transaction_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    pub connection_id: String,
    pub transaction_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    pub task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResponse {
    /// 是否真的向服务端发出了取消指令（任务不存在或已结束时为 false）
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteSqlResponse {
    pub task_id: String,
    /// 按执行顺序排列的语句结果
    pub results: Vec<StatementResult>,
    /// 首个失败语句的信息；为 null 表示全部成功
    pub error: Option<StatementError>,
    pub elapsed_ms: u64,
    /// 面向用户的执行摘要
    pub message: String,
    pub truncated: bool,
    pub cancelled: bool,
    /// 写入查询历史的记录 ID（历史关闭或写入失败时为 null）
    pub history_id: Option<String>,
}

/// 截断语句文本，供标签与历史列表展示
pub fn preview(sql: &str) -> String {
    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= SQL_PREVIEW_LIMIT {
        return compact;
    }
    let truncated: String = compact.chars().take(SQL_PREVIEW_LIMIT).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_value_omits_value_field() {
        let value = serde_json::to_value(DatabaseValue::null()).expect("应可序列化");

        assert_eq!(value["type"], "null");
        assert!(value.get("value").is_none());
    }

    #[test]
    fn integer_kept_as_string() {
        let value = serde_json::to_value(DatabaseValue::text("integer", "18446744073709551615"))
            .expect("应可序列化");

        assert_eq!(value["type"], "integer");
        assert_eq!(value["value"], "18446744073709551615");
    }

    #[test]
    fn max_rows_is_clamped() {
        let base = ExecuteSqlRequest {
            connection_id: "c".into(),
            database_name: None,
            sql: "SELECT 1".into(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        };
        assert_eq!(base.normalized_max_rows(), DEFAULT_MAX_ROWS);

        let huge = ExecuteSqlRequest {
            max_rows: Some(999_999),
            ..base.clone()
        };
        assert_eq!(huge.normalized_max_rows(), MAX_ROWS_LIMIT);
    }

    #[test]
    fn preview_collapses_whitespace_and_truncates() {
        assert_eq!(preview("SELECT  a,\n   b\nFROM t"), "SELECT a, b FROM t");

        let long = "a".repeat(SQL_PREVIEW_LIMIT + 10);
        let result = preview(&long);
        assert!(result.ends_with('…'));
        assert_eq!(result.chars().count(), SQL_PREVIEW_LIMIT + 1);
    }
}
