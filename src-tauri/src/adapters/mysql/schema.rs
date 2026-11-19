//! MySQL 表结构查询：字段、索引、建表 SQL。

use sqlx::{MySqlPool, Row};

use super::quote_identifier;
use crate::errors::{error_code, AppError};
use crate::models::database::{ColumnInfo, IndexInfo, TableSchema};

pub async fn table_schema(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<TableSchema, AppError> {
    let columns = list_columns(pool, database, table).await?;
    if columns.is_empty() {
        return Err(AppError::new(
            error_code::TABLE_NOT_FOUND,
            format!("表 {database}.{table} 不存在或当前账号无权限"),
        ));
    }

    let indexes = list_indexes(pool, database, table).await?;
    let ddl = show_create_table(pool, database, table).await?;

    Ok(TableSchema {
        columns,
        indexes,
        ddl,
    })
}

/// 读取建库语句（`SHOW CREATE DATABASE`）。
pub async fn database_ddl(pool: &MySqlPool, database: &str) -> Result<String, AppError> {
    let statement = format!("SHOW CREATE DATABASE {}", quote_identifier(database)?);

    let row = sqlx::query(&statement)
        .fetch_one(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::DATABASE_NOT_FOUND,
                format!("读取建库 SQL 失败：{error}"),
            )
        })?;

    row.try_get::<String, _>(1)
        .map_err(|error| AppError::storage(format!("解析建库 SQL 失败：{error}")))
}

#[derive(sqlx::FromRow)]
struct ColumnRow {
    name: String,
    column_type: String,
    is_nullable: String,
    default_value: Option<String>,
    column_key: String,
    extra: String,
    comment: String,
    position: u64,
}

async fn list_columns(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, AppError> {
    let rows: Vec<ColumnRow> = sqlx::query_as(
        "SELECT CAST(COLUMN_NAME AS CHAR) AS name,
                CAST(COLUMN_TYPE AS CHAR) AS column_type,
                CAST(IS_NULLABLE AS CHAR) AS is_nullable,
                CAST(COLUMN_DEFAULT AS CHAR) AS default_value,
                CAST(COLUMN_KEY AS CHAR) AS column_key,
                CAST(EXTRA AS CHAR) AS extra,
                CAST(COLUMN_COMMENT AS CHAR) AS comment,
                ORDINAL_POSITION AS position
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
        .map(|row| ColumnInfo {
            name: row.name,
            column_type: row.column_type,
            nullable: row.is_nullable.eq_ignore_ascii_case("YES"),
            default_value: row.default_value,
            column_key: row.column_key,
            extra: row.extra,
            comment: row.comment,
            position: row.position,
        })
        .collect())
}

async fn list_indexes(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, AppError> {
    let rows: Vec<(String, i64, u64, Option<String>, String)> = sqlx::query_as(
        "SELECT CAST(INDEX_NAME AS CHAR),
                NON_UNIQUE,
                SEQ_IN_INDEX,
                CAST(COLUMN_NAME AS CHAR),
                CAST(INDEX_TYPE AS CHAR)
         FROM information_schema.STATISTICS
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
         ORDER BY INDEX_NAME, SEQ_IN_INDEX",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::new(
            error_code::TABLE_NOT_FOUND,
            format!("读取索引信息失败：{error}"),
        )
    })?;

    // 同一索引的多行按顺序合并为字段组合
    let mut indexes: Vec<IndexInfo> = Vec::new();
    for (name, non_unique, _seq, column_name, index_type) in rows {
        let column = column_name.unwrap_or_default();
        match indexes.last_mut() {
            Some(last) if last.name == name => last.columns.push(column),
            _ => indexes.push(IndexInfo {
                name,
                unique: non_unique == 0,
                index_type,
                columns: vec![column],
            }),
        }
    }

    Ok(indexes)
}

async fn show_create_table(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<String, AppError> {
    let statement = format!(
        "SHOW CREATE TABLE {}.{}",
        quote_identifier(database)?,
        quote_identifier(table)?
    );

    let row = sqlx::query(&statement)
        .fetch_one(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::TABLE_NOT_FOUND,
                format!("读取建表 SQL 失败：{error}"),
            )
        })?;

    use sqlx::Row;
    row.try_get::<String, _>(1)
        .map_err(|error| AppError::internal(format!("解析建表 SQL 结果失败：{error}")))
}

/// 公开给导出（dump）使用的建表语句，内部复用 `SHOW CREATE TABLE`。
pub async fn table_create_sql(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<String, AppError> {
    show_create_table(pool, database, table).await
}

#[cfg(test)]
mod tests {
    use super::super::quote_identifier;

    #[test]
    fn ddl_statement_uses_quoted_identifiers() {
        let statement = format!(
            "SHOW CREATE TABLE {}.{}",
            quote_identifier("tbdock").expect("库名应可引用"),
            quote_identifier("user`log").expect("表名应可引用")
        );

        assert_eq!(statement, "SHOW CREATE TABLE `tbdock`.`user``log`");
    }
}
