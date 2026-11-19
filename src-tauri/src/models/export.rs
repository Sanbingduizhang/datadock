//! 数据导出模型。

use serde::{Deserialize, Serialize};

/// 单次导出最多写入的行数，避免误操作把超大表写成几十 GB
pub const MAX_EXPORT_ROWS: u64 = 1_000_000;

/// 每批从服务端读取的行数
pub const EXPORT_BATCH_SIZE: u32 = 1_000;

/// 查询结果导出的行数上限（结果无法分页，只能整体读取，因此低于表导出）
pub const MAX_QUERY_EXPORT_ROWS: u32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTableRequest {
    pub connection_id: String,
    pub database_name: String,
    pub table_name: String,
    pub format: ExportFormat,
    /// 目标文件绝对路径（由保存对话框提供）
    pub path: String,
    pub filter: Option<String>,
    pub order_by: Option<String>,
    pub order_desc: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQueryRequest {
    pub connection_id: String,
    pub database_name: Option<String>,
    pub sql: String,
    pub format: ExportFormat,
    /// 目标文件绝对路径（由保存对话框提供）
    pub path: String,
    pub max_rows: Option<u32>,
}

impl ExportQueryRequest {
    pub fn normalized_max_rows(&self) -> u32 {
        self.max_rows
            .unwrap_or(MAX_QUERY_EXPORT_ROWS)
            .clamp(1, MAX_QUERY_EXPORT_ROWS)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub path: String,
    pub rows_written: u64,
    pub elapsed_ms: u64,
    /// 是否达到行数上限而提前结束
    pub truncated: bool,
}

/// 数据库级导出内容：结构 / 数据 / 结构+数据
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DumpMode {
    /// 仅结构（DDL）
    Structure,
    /// 仅数据（INSERT）
    Data,
    /// 结构 + 数据
    Both,
}

impl DumpMode {
    pub fn includes_structure(self) -> bool {
        matches!(self, DumpMode::Structure | DumpMode::Both)
    }

    pub fn includes_data(self) -> bool {
        matches!(self, DumpMode::Data | DumpMode::Both)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpDatabaseRequest {
    pub connection_id: String,
    pub database_name: String,
    /// 空表示全部表；否则仅导出指定表
    pub tables: Option<Vec<String>>,
    pub mode: DumpMode,
    /// 目标 SQL 文件绝对路径
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpDatabaseResponse {
    pub path: String,
    pub tables: u32,
    pub rows_written: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDatabaseRequest {
    pub connection_id: String,
    /// 目标库（MySQL 用；PG 作为 schema 前缀上下文）
    pub database_name: Option<String>,
    /// 源 SQL 文件绝对路径
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDatabaseResponse {
    pub statements: u32,
    pub elapsed_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_parses_lowercase() {
        assert_eq!(
            serde_json::from_str::<ExportFormat>("\"csv\"").expect("应可解析"),
            ExportFormat::Csv
        );
        assert_eq!(ExportFormat::Json.extension(), "json");
    }
}
