//! MySQL 元数据查询：数据库列表、表/视图列表。
//!
//! 注意：MySQL 8.4 的 information_schema 部分文本列返回 BINARY，
//! 必须 `CAST(... AS CHAR)`，否则驱动解码失败（见 TDR-001）。

use sqlx::MySqlPool;

use crate::errors::{error_code, AppError};
use crate::models::database::{DatabaseInfo, TableInfo, TableKind};

pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<DatabaseInfo>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT CAST(SCHEMA_NAME AS CHAR)
         FROM information_schema.SCHEMATA
         ORDER BY SCHEMA_NAME",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::new(
            error_code::DATABASE_NOT_FOUND,
            format!("读取数据库列表失败：{error}"),
        )
    })?;

    Ok(rows
        .into_iter()
        .map(|(name,)| DatabaseInfo { name })
        .collect())
}

pub async fn list_tables(pool: &MySqlPool, database: &str) -> Result<Vec<TableInfo>, AppError> {
    let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT CAST(TABLE_NAME AS CHAR),
                CAST(TABLE_TYPE AS CHAR),
                CAST(ENGINE AS CHAR),
                CAST(TABLE_COMMENT AS CHAR)
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA = ?
         ORDER BY TABLE_NAME",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::new(
            error_code::TABLE_NOT_FOUND,
            format!("读取表列表失败：{error}"),
        )
    })?;

    Ok(rows
        .into_iter()
        .map(|(name, table_type, engine, comment)| TableInfo {
            name,
            table_kind: if table_type.eq_ignore_ascii_case("VIEW") {
                TableKind::View
            } else {
                TableKind::Table
            },
            engine,
            comment: comment.filter(|text| !text.is_empty()),
        })
        .collect())
}
