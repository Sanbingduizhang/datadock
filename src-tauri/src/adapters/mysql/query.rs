//! MySQL 单条 SQL 执行。
//!
//! 语句的切分与多语句循环在 Service 层完成，这里只负责执行一条语句并转换结果。
//! 结果行数通过会话变量 `sql_select_limit` 限制，不改写用户 SQL（见 TDR-002）。

use sqlx::mysql::MySqlRow;
use sqlx::types::chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use sqlx::types::BigDecimal;
use sqlx::{Column, Executor, MySqlPool, Row, TypeInfo, ValueRef};

use crate::errors::{error_code, AppError};
use crate::models::query::{DatabaseValue, QueryColumn, QueryResultSet};

/// 单条语句的执行产物（不含语句序号与耗时，由 Service 层组装）
pub struct StatementOutcome {
    pub result_set: Option<QueryResultSet>,
    pub affected_rows: Option<u64>,
    pub last_insert_id: Option<String>,
    pub message: String,
    pub truncated: bool,
}

/// 判断语句是否返回结果集
pub fn returns_rows(sql: &str) -> bool {
    let mut keyword = String::new();
    for ch in sql.trim().chars() {
        if ch.is_alphabetic() {
            keyword.push(ch.to_ascii_uppercase());
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
        "SELECT" | "SHOW" | "DESC" | "DESCRIBE" | "EXPLAIN" | "WITH" | "TABLE" | "VALUES"
    )
}

pub async fn execute_one(
    pool: &MySqlPool,
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
    pool: &MySqlPool,
    sql: &str,
    max_rows: u32,
) -> Result<StatementOutcome, AppError> {
    // 会话级行数上限：不改写用户 SQL，同时让内存占用有界。
    // 执行会话是 max_connections = 1 的独立连接池，因此 SET 一定作用于真正执行 SQL 的连接。
    let limit_statement = format!(
        "SET SESSION sql_select_limit = {}",
        max_rows.saturating_add(1)
    );
    sqlx::raw_sql(&limit_statement)
        .execute(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("设置结果行数上限失败：{error}"),
            )
        })?;

    let mut columns: Vec<QueryColumn> = Vec::new();

    let fetched = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::new(error_code::SQL_EXECUTION_FAILED, error.to_string()))?;

    // 空结果集没有行可读列名，用 describe 补齐表头（只读元数据，不会执行语句）
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
        last_insert_id: None,
        message,
        truncated,
    })
}

async fn execute_statement(pool: &MySqlPool, sql: &str) -> Result<StatementOutcome, AppError> {
    let result = sqlx::raw_sql(sql)
        .execute(pool)
        .await
        .map_err(|error| AppError::new(error_code::SQL_EXECUTION_FAILED, error.to_string()))?;

    let affected_rows = result.rows_affected();
    let insert_id = result.last_insert_id();

    Ok(StatementOutcome {
        result_set: None,
        affected_rows: Some(affected_rows),
        last_insert_id: (insert_id > 0).then(|| insert_id.to_string()),
        message: format!("影响 {affected_rows} 行"),
        truncated: false,
    })
}

/// 从结果行读取列名与驱动类型（空结果集时无列可读，调用方需另行处理）
pub(crate) fn columns_of(row: &MySqlRow) -> Vec<QueryColumn> {
    row.columns()
        .iter()
        .map(|column| QueryColumn {
            name: column.name().to_string(),
            type_name: column.type_info().name().to_string(),
        })
        .collect()
}

pub(crate) fn convert_row(row: &MySqlRow, columns: &[QueryColumn]) -> Vec<DatabaseValue> {
    (0..columns.len())
        .map(|index| convert_value(row, index, &columns[index].type_name))
        .collect()
}

fn convert_value(row: &MySqlRow, index: usize, type_name: &str) -> DatabaseValue {
    let is_null = row
        .try_get_raw(index)
        .map(|value| value.is_null())
        .unwrap_or(true);
    if is_null {
        return DatabaseValue::null();
    }

    let upper = type_name.to_ascii_uppercase();

    // 无符号大整数必须按 u64 读取，避免溢出
    if upper.contains("UNSIGNED") {
        if let Ok(value) = row.try_get::<u64, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
    }

    if upper.contains("INT") {
        if let Ok(value) = row.try_get::<i64, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
        if let Ok(value) = row.try_get::<u64, _>(index) {
            return DatabaseValue::text("integer", value.to_string());
        }
    }

    if upper.contains("DECIMAL") {
        if let Ok(value) = row.try_get::<BigDecimal, _>(index) {
            return DatabaseValue::text("decimal", value.to_string());
        }
    }

    if upper.starts_with("DOUBLE") || upper.starts_with("FLOAT") {
        if let Ok(value) = row.try_get::<f64, _>(index) {
            return DatabaseValue::text("float", value.to_string());
        }
    }

    if upper == "BOOLEAN" {
        if let Ok(value) = row.try_get::<i8, _>(index) {
            return DatabaseValue::text("boolean", if value == 0 { "false" } else { "true" });
        }
    }

    if upper.contains("JSON") {
        if let Ok(value) = row.try_get::<serde_json::Value, _>(index) {
            return DatabaseValue::text("json", value.to_string());
        }
    }

    if upper.contains("DATETIME") || upper.contains("TIMESTAMP") {
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

    if upper == "TIME" {
        if let Ok(value) = row.try_get::<NaiveTime, _>(index) {
            return DatabaseValue::text("string", value.to_string());
        }
    }

    if upper.contains("BLOB") || upper.contains("BINARY") || upper.contains("BIT") {
        if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
            return DatabaseValue::text("binary", format!("<binary {} bytes>", value.len()));
        }
    }

    if let Ok(value) = row.try_get::<String, _>(index) {
        return DatabaseValue::text("string", value);
    }

    if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
        return DatabaseValue::text("binary", format!("<binary {} bytes>", value.len()));
    }

    DatabaseValue::text("string", "<无法解码的值>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_result_returning_statements() {
        assert!(returns_rows("SELECT 1"));
        assert!(returns_rows("  select * from t"));
        assert!(returns_rows("SHOW TABLES"));
        assert!(returns_rows("WITH c AS (SELECT 1) SELECT * FROM c"));
        assert!(returns_rows("EXPLAIN SELECT 1"));

        assert!(!returns_rows("INSERT INTO t VALUES (1)"));
        assert!(!returns_rows("UPDATE t SET a = 1"));
        assert!(!returns_rows("CREATE TABLE t (id INT)"));
        assert!(!returns_rows(""));
    }
}
