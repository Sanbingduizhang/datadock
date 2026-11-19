//! 表数据浏览模型。

use serde::{Deserialize, Serialize};

use super::query::{DatabaseValue, QueryColumn};

pub const DEFAULT_PAGE_SIZE: u32 = 100;
pub const MAX_PAGE_SIZE: u32 = 1_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataRequest {
    pub connection_id: String,
    pub database_name: String,
    pub table_name: String,
    /// 页码，从 1 开始
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    /// 排序列；缺省时使用主键（无主键则按存储顺序）
    pub order_by: Option<String>,
    pub order_desc: Option<bool>,
    /// 过滤条件（`WHERE` 之后的表达式），例如 `status = 1 AND name LIKE 'a%'`
    pub filter: Option<String>,
}

impl TableDataRequest {
    pub fn normalized_page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn normalized_page_size(&self) -> u32 {
        self.page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataResponse {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<DatabaseValue>>,
    /// 表的总行数（COUNT(*)；大表可能较慢）
    pub total_rows: u64,
    pub page: u32,
    pub page_size: u32,
    pub elapsed_ms: u64,
    /// 主键列，用于默认排序与后续行编辑定位
    pub primary_key: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_and_size_are_clamped() {
        let request = TableDataRequest {
            connection_id: "c".into(),
            database_name: "tbdock".into(),
            table_name: "users".into(),
            page: Some(0),
            page_size: Some(99_999),
            order_by: None,
            order_desc: None,
            filter: None,
        };

        assert_eq!(request.normalized_page(), 1);
        assert_eq!(request.normalized_page_size(), MAX_PAGE_SIZE);
    }
}
