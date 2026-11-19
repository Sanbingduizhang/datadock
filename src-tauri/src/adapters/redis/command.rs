//! Redis 命令执行与结果转换。

use std::time::Instant;

use redis::aio::MultiplexedConnection;
use redis::Value;

use super::parse_command;
use crate::errors::{error_code, AppError};
use crate::models::query::{
    DatabaseValue, ExecuteSqlResponse, QueryColumn, QueryResultSet, StatementResult,
};

pub async fn execute(
    connection: &mut MultiplexedConnection,
    command_line: &str,
) -> Result<ExecuteSqlResponse, AppError> {
    let started = Instant::now();
    let args = parse_command(command_line)?;

    let mut command = redis::cmd(&args[0]);
    for argument in &args[1..] {
        command.arg(argument);
    }

    let value: Value = command.query_async(connection).await.map_err(|error| {
        AppError::new(
            error_code::SQL_EXECUTION_FAILED,
            format!("Redis 执行失败：{error}"),
        )
    })?;

    // HGETALL / CONFIG GET 在 RESP2 下返回扁平数组（k1,v1,k2,v2），按两列展示更直观
    let prefer_pairs = matches!(
        args[0].to_ascii_uppercase().as_str(),
        "HGETALL" | "CONFIG" | "HSCAN"
    );
    let (columns, rows, summary) = convert(value, prefer_pairs);
    let row_count = rows.len();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(ExecuteSqlResponse {
        task_id: String::new(),
        results: vec![StatementResult {
            index: 1,
            sql_preview: command_line.trim().to_string(),
            result_set: Some(QueryResultSet {
                columns,
                rows,
                row_count,
                truncated: false,
            }),
            affected_rows: None,
            last_insert_id: None,
            elapsed_ms,
            message: summary.clone(),
        }],
        error: None,
        elapsed_ms,
        message: format!("{summary} · {elapsed_ms}ms"),
        truncated: false,
        cancelled: false,
        history_id: None,
    })
}

fn text_column(name: &str) -> QueryColumn {
    QueryColumn {
        name: name.to_string(),
        type_name: "string".to_string(),
    }
}

fn text(value: impl Into<String>) -> DatabaseValue {
    DatabaseValue::text("string", value)
}

fn convert(
    value: Value,
    prefer_pairs: bool,
) -> (Vec<QueryColumn>, Vec<Vec<DatabaseValue>>, String) {
    match value {
        Value::Nil => (
            vec![text_column("result")],
            Vec::new(),
            "结果为 nil".to_string(),
        ),
        Value::Int(number) => (
            vec![text_column("result")],
            vec![vec![DatabaseValue::text("integer", number.to_string())]],
            "返回 1 行".to_string(),
        ),
        Value::Array(items) => convert_array(items, prefer_pairs),
        Value::Map(pairs) => {
            let rows = pairs
                .into_iter()
                .map(|(key, item)| vec![text(format_value(&key)), text(format_value(&item))])
                .collect::<Vec<_>>();
            (
                vec![text_column("field"), text_column("value")],
                rows,
                "返回键值对".to_string(),
            )
        }
        Value::Set(items) => convert_array(items, false),
        other => (
            vec![text_column("result")],
            vec![vec![text(format_value(&other))]],
            "返回 1 行".to_string(),
        ),
    }
}

fn convert_array(
    items: Vec<Value>,
    prefer_pairs: bool,
) -> (Vec<QueryColumn>, Vec<Vec<DatabaseValue>>, String) {
    if items.is_empty() {
        return (
            vec![text_column("value")],
            Vec::new(),
            "结果为空列表".to_string(),
        );
    }

    let nested_pairs = items.iter().all(|item| match item {
        Value::Array(pair) => pair.len() == 2,
        _ => false,
    });

    let flat_pairs = prefer_pairs
        && items.len() % 2 == 0
        && items
            .iter()
            .all(|item| !matches!(item, Value::Array(_) | Value::Map(_)));

    let all_pairs = nested_pairs || flat_pairs;

    if all_pairs {
        let rows: Vec<Vec<DatabaseValue>> = if flat_pairs {
            items
                .chunks(2)
                .map(|pair| vec![text(format_value(&pair[0])), text(format_value(&pair[1]))])
                .collect()
        } else {
            items
                .into_iter()
                .map(|item| match item {
                    Value::Array(pair) => {
                        vec![text(format_value(&pair[0])), text(format_value(&pair[1]))]
                    }
                    other => vec![text(format_value(&other)), text("")],
                })
                .collect()
        };

        return (
            vec![text_column("field"), text_column("value")],
            rows,
            "返回键值对".to_string(),
        );
    }

    let rows = items
        .into_iter()
        .map(|item| vec![text(format_value(&item))])
        .collect::<Vec<_>>();

    (vec![text_column("value")], rows, "返回列表".to_string())
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Nil => "(nil)".to_string(),
        Value::Int(number) => number.to_string(),
        Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        Value::SimpleString(text) => text.clone(),
        Value::VerbatimString { text, .. } => text.clone(),
        Value::Okay => "OK".to_string(),
        Value::ServerError(error) => format!("ERR {error:?}"),
        Value::Double(number) => number.to_string(),
        Value::Boolean(flag) => flag.to_string(),
        Value::BigNumber(number) => number.to_string(),
        Value::Array(items) | Value::Set(items) | Value::Push { data: items, .. } => {
            let parts: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Attribute { data, .. } => format_value(data),
        Value::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(key, item)| format!("{}: {}", format_value(key), format_value(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_scalar_values() {
        assert_eq!(format_value(&Value::Int(3)), "3");
        assert_eq!(format_value(&Value::Okay), "OK");
        assert_eq!(format_value(&Value::Nil), "(nil)");
        assert_eq!(
            format_value(&Value::Array(vec![Value::Int(1), Value::Int(2)])),
            "[1, 2]"
        );
    }

    #[test]
    fn pair_arrays_become_two_columns() {
        let value = Value::Array(vec![
            Value::Array(vec![
                Value::BulkString(b"a".to_vec()),
                Value::BulkString(b"1".to_vec()),
            ]),
            Value::Array(vec![
                Value::BulkString(b"b".to_vec()),
                Value::BulkString(b"2".to_vec()),
            ]),
        ]);

        let (columns, rows, _) = convert(value, false);
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].name, "field");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1].value.as_deref(), Some("2"));
    }
}
