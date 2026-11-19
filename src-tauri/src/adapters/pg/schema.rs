//! PostgreSQL 表结构：字段、索引、建表语句与标识符转义。

use sqlx::PgPool;

use crate::errors::{error_code, AppError};
use crate::models::database::{ColumnInfo, IndexInfo, TableSchema};

pub fn quote_identifier(name: &str) -> Result<String, AppError> {
    if name.is_empty() || name.len() > 128 {
        return Err(AppError::validation("标识符长度非法"));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || name
            .chars()
            .next()
            .map(|ch| ch.is_ascii_digit())
            .unwrap_or(false)
    {
        return Err(AppError::validation(format!("标识符包含非法字符：{name}")));
    }
    Ok(format!("\"{name}\""))
}

pub async fn table_schema(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<TableSchema, AppError> {
    let columns = column_info(pool, schema, table).await?;
    let indexes = index_info(pool, schema, table).await?;
    let ddl = build_ddl(pool, schema, table).await?;

    Ok(TableSchema {
        columns,
        indexes,
        ddl,
    })
}

// information_schema 一次返回 7 列，元组仅在此处内部使用，不再额外引入映射结构体。
#[allow(clippy::type_complexity)]
pub(crate) async fn column_info(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, AppError> {
    let rows: Vec<(String, String, String, Option<String>, Option<String>, String, i32)> = sqlx::query_as(
        "SELECT c.column_name, \
                c.data_type, \
                c.is_nullable, \
                c.column_default, \
                (SELECT pg_catalog.col_description((quote_ident(c.table_schema)||'.'||quote_ident(c.table_name))::regclass::oid, c.ordinal_position)) AS comment, \
                COALESCE((SELECT 'YES' FROM information_schema.key_column_usage k \
                          JOIN information_schema.table_constraints tc \
                            ON tc.constraint_name = k.constraint_name \
                           AND tc.constraint_schema = k.constraint_schema \
                          WHERE tc.constraint_type = 'PRIMARY KEY' \
                            AND tc.table_schema = c.table_schema \
                            AND tc.table_name = c.table_name \
                            AND k.table_schema = c.table_schema \
                            AND k.table_name = c.table_name \
                            AND k.column_name = c.column_name), 'NO') AS is_pk, \
                c.ordinal_position \
         FROM information_schema.columns c \
         WHERE c.table_schema = $1 AND c.table_name = $2 \
         ORDER BY c.ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| AppError::storage(format!("读取字段信息失败：{error}")))?;

    Ok(rows
        .into_iter()
        .map(
            |(name, data_type, nullable, default_value, comment, is_pk, ordinal_position)| {
                ColumnInfo {
                    name,
                    column_type: data_type,
                    nullable: nullable.eq_ignore_ascii_case("YES"),
                    default_value,
                    column_key: if is_pk.eq_ignore_ascii_case("YES") {
                        "PRI"
                    } else {
                        ""
                    }
                    .to_string(),
                    extra: String::new(),
                    comment: comment.unwrap_or_default(),
                    position: ordinal_position.max(0) as u64,
                }
            },
        )
        .collect())
}

async fn index_info(pool: &PgPool, schema: &str, table: &str) -> Result<Vec<IndexInfo>, AppError> {
    let rows: Vec<(String, bool, String)> = sqlx::query_as(
        "SELECT i.relname AS index_name, \
                ix.indisunique, \
                am.amname AS method \
         FROM pg_index ix \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_am am ON am.oid = i.relam \
         WHERE n.nspname = $1 AND t.relname = $2 \
         ORDER BY i.relname",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| AppError::storage(format!("读取索引信息失败：{error}")))?;

    let mut result = Vec::with_capacity(rows.len());
    for (name, unique, method) in rows {
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT a.attname FROM pg_index ix \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true \
             JOIN pg_attribute a ON a.attrelid = ix.indrelid AND a.attnum = k.attnum \
             WHERE i.relname = $1 \
             ORDER BY k.ord",
        )
        .bind(&name)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        result.push(IndexInfo {
            name,
            unique,
            index_type: method,
            columns,
        });
    }

    Ok(result)
}

async fn build_ddl(pool: &PgPool, schema: &str, table: &str) -> Result<String, AppError> {
    // 普通表 pg_get_viewdef 返回 NULL，视图返回定义文本
    let view_row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT pg_get_viewdef(to_regclass($1)::oid, true)")
            .bind(format!("{schema}.{table}"))
            .fetch_optional(pool)
            .await
            .map_err(|error| AppError::storage(format!("判定视图失败：{error}")))?;

    if let Some((Some(view_def),)) = view_row {
        if !view_def.trim().is_empty() {
            return Ok(format!(
                "CREATE OR REPLACE VIEW {schema}.{table} AS\n{view_def}"
            ));
        }
    }

    let metadata = column_info(pool, schema, table).await?;
    if metadata.is_empty() {
        return Err(AppError::new(
            error_code::DATABASE_NOT_FOUND,
            format!("表 {schema}.{table} 不存在"),
        ));
    }

    let mut lines = Vec::with_capacity(metadata.len());
    for column in &metadata {
        let mut line = format!(
            "  {} {}",
            quote_identifier(&column.name)?,
            column.column_type
        );
        if !column.nullable {
            line.push_str(" NOT NULL");
        }
        if let Some(default) = column.default_value.as_deref() {
            line.push_str(&format!(" DEFAULT {default}"));
        }
        if !column.comment.is_empty() {
            line.push_str(&format!(" -- {}", column.comment));
        }
        lines.push(line);
    }

    // 主键与索引
    let indexes = index_info(pool, schema, table).await?;
    for index in &indexes {
        let columns = index
            .columns
            .iter()
            .map(|column| quote_identifier(column).unwrap_or_else(|_| column.clone()))
            .collect::<Vec<_>>()
            .join(", ");

        if index.index_type == "btree" && index.columns.len() == 1 {
            // 主键由列元数据体现，无需重复
            continue;
        }

        let unique = if index.unique { "UNIQUE " } else { "" };
        lines.push(format!(
            "  {unique}INDEX {} USING {} ({columns})",
            quote_identifier(&index.name)?,
            index.index_type
        ));
    }

    let table_quoted = quote_identifier(table)?;
    Ok(format!(
        "CREATE TABLE {}.{} (\n{}\n);",
        quote_identifier(schema)?,
        table_quoted,
        lines.join(",\n")
    ))
}

pub async fn database_ddl(pool: &PgPool) -> Result<String, AppError> {
    let row: (String,) = sqlx::query_as("SELECT current_database()")
        .fetch_one(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取当前数据库失败：{error}")))?;

    Ok(format!(
        "-- PostgreSQL 数据库 {0} 是物理库，无法直接导出 CREATE DATABASE 语句。\n\
         -- 可使用 pg_dump --schema-only 导出完整结构。\n\
         CREATE DATABASE \"{0}\";",
        row.0
    ))
}
