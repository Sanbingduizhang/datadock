//! PostgreSQL 资源树：schema 列表与表列表。

use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::database::{TableInfo, TableKind};

/// 列出全部用户 schema（排除系统 schema）。
pub async fn list_schemas(pool: &PgPool) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT nspname FROM pg_namespace \
         WHERE nspname NOT LIKE 'pg_%' AND nspname != 'information_schema' \
         ORDER BY CASE WHEN nspname = 'public' THEN 0 ELSE 1 END, nspname",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| AppError::storage(format!("读取 schema 列表失败：{error}")))?;

    Ok(rows.into_iter().map(|row| row.0).collect())
}

/// 列出 schema 下的表与视图（含注释）。
pub async fn list_tables(pool: &PgPool, schema: &str) -> Result<Vec<TableInfo>, AppError> {
    if schema.trim().is_empty() {
        return Err(AppError::validation("schema 名称不能为空"));
    }

    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT c.relname, \
                CASE WHEN c.relkind IN ('v', 'm') THEN 'VIEW' ELSE 'BASE TABLE' END AS kind, \
                obj_description(c.oid, 'pg_class') AS comment \
         FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p', 'v', 'm') \
           AND c.relispartition = false \
         ORDER BY c.relkind, c.relname",
    )
    .bind(schema)
    .fetch_all(pool)
    .await
    .map_err(|error| AppError::storage(format!("读取表列表失败：{error}")))?;

    Ok(rows
        .into_iter()
        .map(|(name, kind, comment)| TableInfo {
            name,
            table_kind: if kind == "VIEW" {
                TableKind::View
            } else {
                TableKind::Table
            },
            engine: None,
            comment,
        })
        .collect())
}
