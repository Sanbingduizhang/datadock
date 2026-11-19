//! PostgreSQL 单条 SQL 执行与结果转换。

use sqlx::postgres::PgRow;
use sqlx::types::chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::types::{BigDecimal, JsonValue};
use sqlx::{Column, Executor, PgPool, Row, TypeInfo, ValueRef};

use crate::errors::{error_code, AppError};
use crate::models::query::{DatabaseValue, QueryColumn, QueryResultSet};

pub struct StatementOutcome {
    pub result_set: Option<QueryResultSet>,
    pub affected_rows: Option<u64>,
    pub message: String,
    pub truncated: bool,
}

pub fn returns_rows(sql: &str) -> bool {
    let mut keyword = String::new();
    for ch in sql.trim().chars() {
        if ch.is_alphabetic() {
            keyword.push(ch.to_ascii_lowercase());
        } else if !keyword.is_empty() {
            break;
        } else if ch.is_whitespace() || ch == '(' {
            continue;
        } else {
            break;
        }
    }

    matches!(
        keyword.as_str(),
        "select"
            | "show"
            | "desc"
            | "describe"
            | "explain"
            | "with"
            | "table"
            | "values"
            | "fetch"
            | "copy"
    )
}

pub async fn execute_one(
    pool: &PgPool,
    sql: &str,
    max_rows: u32,
) -> Result<StatementOutcome, AppError> {
    if returns_rows(sql) {
        execute_query(pool, sql, max_rows).await
    } else {
        execute_statement(pool, sql).await
    }
}

async fn execute_query(
    pool: &PgPool,
    sql: &str,
    max_rows: u32,
) -> Result<StatementOutcome, AppError> {
    // PG 不会话级 LIMIT，改写 SQL 包一层：只读元数据时不需要这个逻辑
    let wrapped = format!(
        "SELECT * FROM ({}) AS _sub LIMIT {}",
        sql.trim().trim_end_matches(';'),
        max_rows.saturating_add(1)
    );

    let fetched = sqlx::query(&wrapped)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::new(error_code::SQL_EXECUTION_FAILED, error.to_string()))?;

    let mut columns: Vec<QueryColumn> = Vec::new();

    // 空结果集用 describe 补列头
    if columns.is_empty() {
        if let Ok(described) = pool.describe(sql).await {
            columns = described
                .columns
                .iter()
                .map(|column| QueryColumn {
                    name: column.name().to_string(),
                    type_name: column.type_info().name().to_string(),
                })
                .collect();
        }
    }

    let truncated = fetched.len() > max_rows as usize;
    let mut rows: Vec<Vec<DatabaseValue>> =
        Vec::with_capacity(fetched.len().min(max_rows as usize));

    for row in fetched.into_iter().take(max_rows as usize) {
        if columns.is_empty() {
            columns = columns_of(&row);
        }
        rows.push(convert_row(&row, &columns));
    }

    let row_count = rows.len();
    let message = if truncated {
        format!("已返回 {row_count} 行（达到行数上限，结果可能被截断）")
    } else {
        format!("返回 {row_count} 行")
    };

    Ok(StatementOutcome {
        result_set: Some(QueryResultSet {
            columns,
            rows,
            row_count,
            truncated,
        }),
        affected_rows: None,
        message,
        truncated,
    })
}

async fn execute_statement(pool: &PgPool, sql: &str) -> Result<StatementOutcome, AppError> {
    let result = sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(|error| AppError::new(error_code::SQL_EXECUTION_FAILED, error.to_string()))?;

    let affected_rows = result.rows_affected();

    Ok(StatementOutcome {
        result_set: None,
        affected_rows: Some(affected_rows),
        message: format!("影响 {affected_rows} 行"),
        truncated: false,
    })
}

pub(crate) fn columns_of(row: &PgRow) -> Vec<QueryColumn> {
    row.columns()
        .iter()
        .map(|column| QueryColumn {
            name: column.name().to_string(),
            type_name: column.type_info().name().to_string(),
        })
        .collect()
}

pub(crate) fn convert_row(row: &PgRow, columns: &[QueryColumn]) -> Vec<DatabaseValue> {
    (0..columns.len())
        .map(|index| convert_value(row, index, &columns[index].type_name))
        .collect()
}

fn convert_value(row: &PgRow, index: usize, type_name: &str) -> DatabaseValue {
    let is_null = row
        .try_get_raw(index)
        .map(|value| value.is_null())
        .unwrap_or(true);
    if is_null {
        return DatabaseValue::null();
    }

    let upper = type_name.to_ascii_uppercase();

    // 整数族
    if upper == "INT2" || upper == "SMALLINT" {
        if let Ok(value) = row.try_get::<i16, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
    }
    if upper == "INT4" || upper == "INT" || upper == "INTEGER" {
        if let Ok(value) = row.try_get::<i32, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
    }
    if upper == "INT8" || upper == "BIGINT" {
        if let Ok(value) = row.try_get::<i64, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
    }

    // 数值与浮点
    if upper == "NUMERIC" || upper == "DECIMAL" {
        if let Ok(value) = row.try_get::<BigDecimal, _>(index) {
            return DatabaseValue::text("decimal", value.to_string());
        }
    }
    if upper == "FLOAT4" || upper == "REAL" {
        if let Ok(value) = row.try_get::<f32, _>(index) {
            return DatabaseValue::text("float", value.to_string());
        }
    }
    if upper == "FLOAT8" || upper == "DOUBLE PRECISION" {
        if let Ok(value) = row.try_get::<f64, _>(index) {
            return DatabaseValue::text("float", value.to_string());
        }
    }

    // 布尔
    if upper == "BOOL" || upper == "BOOLEAN" {
        if let Ok(value) = row.try_get::<bool, _>(index) {
            return DatabaseValue::text("boolean", if value { "true" } else { "false" });
        }
    }

    // JSON
    if upper == "JSON" || upper == "JSONB" {
        if let Ok(value) = row.try_get::<JsonValue, _>(index) {
            return DatabaseValue::text("json", value.to_string());
        }
        if let Ok(value) = row.try_get::<String, _>(index) {
            return DatabaseValue::text("json", value);
        }
    }

    // 时间
    if upper == "TIMESTAMPTZ" || upper == "TIMESTAMP WITH TIME ZONE" {
        if let Ok(value) = row.try_get::<DateTime<Utc>, _>(index) {
            return DatabaseValue::text(
                "datetime",
                value.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            );
        }
    }
    if upper == "TIMESTAMP" || upper == "TIMESTAMP WITHOUT TIME ZONE" {
        if let Ok(value) = row.try_get::<NaiveDateTime, _>(index) {
            return DatabaseValue::text(
                "datetime",
                value.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            );
        }
    }
    if upper == "DATE" {
        if let Ok(value) = row.try_get::<NaiveDate, _>(index) {
            return DatabaseValue::text("datetime", value.to_string());
        }
    }
    if upper == "TIME" || upper == "TIMETZ" {
        if let Ok(value) = row.try_get::<NaiveTime, _>(index) {
            return DatabaseValue::text("string", value.to_string());
        }
    }

    // 二进制
    if upper == "BYTEA" {
        if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
            return DatabaseValue::text("binary", format!("<binary {} bytes>", value.len()));
        }
    }

    // UUID
    if upper == "UUID" {
        if let Ok(value) = row.try_get::<String, _>(index) {
            return DatabaseValue::text("string", value);
        }
    }

    // 默认按字符串
    if let Ok(value) = row.try_get::<String, _>(index) {
        return DatabaseValue::text("string", value);
    }
    if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
        return DatabaseValue::text("binary", format!("<binary {} bytes>", value.len()));
    }

    DatabaseValue::text("string", "<无法解码的值>")
}
