//! MySQL 表数据读取（分页）。

use std::time::Instant;

use sqlx::MySqlPool;

use super::{query, quote_identifier};
use crate::errors::{error_code, AppError};
use crate::models::query::QueryColumn;
use crate::models::table::TableDataResponse;

/// 列元数据：名称、完整类型、是否主键
pub(crate) struct ColumnMeta {
    pub(crate) name: String,
    pub(crate) column_type: String,
    pub(crate) primary: bool,
}

/// 读取一页表数据。
///
/// - `order_by` 必须是真实列名（会与元数据校验），缺省时按主键升序，保证翻页稳定；
/// - `filter` 是用户输入的 WHERE 条件（不含 `WHERE` 关键字），由调用方做基础校验，
///   这里只做拼接，不参与 SQL 语义解析。
#[allow(clippy::too_many_arguments)]
pub async fn fetch_data(
    pool: &MySqlPool,
    database: &str,
    table: &str,
    page: u32,
    page_size: u32,
    order_by: Option<&str>,
    order_desc: bool,
    filter: Option<&str>,
) -> Result<TableDataResponse, AppError> {
    let started = Instant::now();
    let table_ref = format!(
        "{}.{}",
        quote_identifier(database)?,
        quote_identifier(table)?
    );

    let metadata = column_metadata(pool, database, table).await?;
    let primary_key: Vec<String> = metadata
        .iter()
        .filter(|column| column.primary)
        .map(|column| column.name.clone())
        .collect();

    if let Some(column) = order_by {
        if !metadata
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(column))
        {
            return Err(AppError::validation(format!("排序列 {column} 不存在")));
        }
    }

    let where_clause = match filter {
        Some(filter) => format!(" WHERE {filter}"),
        None => String::new(),
    };

    // COUNT(*) 在 MySQL 中是带符号 BIGINT
    let total_rows: i64 =
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table_ref}{where_clause}"))
            .fetch_one(pool)
            .await
            .map_err(|error| AppError::validation(format!("筛选条件无效：{error}")))?;

    // 分页必须有稳定顺序，否则翻页会重复/漏行：优先使用排序列或主键
    let sort_column = order_by
        .map(str::to_string)
        .or_else(|| primary_key.first().cloned());

    let order_clause = match sort_column.as_deref() {
        Some(column) => format!(
            " ORDER BY {}{}",
            quote_identifier(column)?,
            if order_desc { " DESC" } else { " ASC" }
        ),
        None => String::new(),
    };

    let offset = u64::from(page.saturating_sub(1)) * u64::from(page_size);
    let sql = format!(
        "SELECT * FROM {table_ref}{where_clause}{order_clause} LIMIT {page_size} OFFSET {offset}"
    );

    let fetched = sqlx::query(&sql).fetch_all(pool).await.map_err(|error| {
        AppError::new(
            error_code::SQL_EXECUTION_FAILED,
            format!("读取表数据失败：{error}"),
        )
    })?;

    let columns: Vec<QueryColumn> = if metadata.is_empty() {
        fetched.first().map(query::columns_of).unwrap_or_default()
    } else {
        metadata
            .iter()
            .map(|column| QueryColumn {
                name: column.name.clone(),
                type_name: column.column_type.clone(),
            })
            .collect()
    };

    let rows = fetched
        .iter()
        .map(|row| query::convert_row(row, &columns))
        .collect();

    Ok(TableDataResponse {
        columns,
        rows,
        total_rows: total_rows.max(0) as u64,
        page,
        page_size,
        elapsed_ms: started.elapsed().as_millis() as u64,
        primary_key,
    })
}

pub(crate) async fn column_metadata(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnMeta>, AppError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT CAST(COLUMN_NAME AS CHAR), CAST(COLUMN_TYPE AS CHAR), CAST(COLUMN_KEY AS CHAR)
         FROM information_schema.COLUMNS
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
         ORDER BY ORDINAL_POSITION",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::new(
            error_code::TABLE_NOT_FOUND,
            format!("读取字段信息失败：{error}"),
        )
    })?;

    Ok(rows
        .into_iter()
        .map(|(name, column_type, column_key)| ColumnMeta {
            name,
            column_type,
            primary: column_key.eq_ignore_ascii_case("PRI"),
        })
        .collect())
}
