//! PostgreSQL 表数据读取（分页、排序、筛选）。
//!
//! 与 MySQL 路径保持一致的语义：
//! - `database` 参数即 schema 名；
//! - 无排序列时按主键升序，保证翻页稳定；
//! - `filter` 是 `WHERE` 之后的表达式，由服务层做基础校验。

use std::time::Instant;

use sqlx::PgPool;

use super::query as pg_query;
use super::schema::{column_info, quote_identifier};
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
#[allow(clippy::too_many_arguments)]
pub async fn fetch_data(
    pool: &PgPool,
    schema: &str,
    table: &str,
    page: u32,
    page_size: u32,
    order_by: Option<&str>,
    order_desc: bool,
    filter: Option<&str>,
) -> Result<TableDataResponse, AppError> {
    let started = Instant::now();
    let table_ref = format!("{}.{}", quote_identifier(schema)?, quote_identifier(table)?);

    let metadata = column_metadata(pool, schema, table).await?;
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

    // PG 的 COUNT(*) 是 bigint，对应 i64
    let total_rows: i64 =
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table_ref}{where_clause}"))
            .fetch_one(pool)
            .await
            .map_err(|error| AppError::validation(format!("筛选条件无效：{error}")))?;

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
        fetched
            .first()
            .map(pg_query::columns_of)
            .unwrap_or_default()
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
        .map(|row| pg_query::convert_row(row, &columns))
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
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnMeta>, AppError> {
    let columns = column_info(pool, schema, table).await?;

    Ok(columns
        .into_iter()
        .map(|column| ColumnMeta {
            primary: column.column_key.eq_ignore_ascii_case("PRI"),
            name: column.name,
            column_type: column.column_type,
        })
        .collect())
}
