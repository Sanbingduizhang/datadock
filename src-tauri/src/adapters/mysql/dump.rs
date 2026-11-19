//! MySQL 数据库级导出（结构 / 数据 / 结构+数据）。
//!
//! 结构走 `SHOW CREATE TABLE`（与建表语句一致），数据按批读取拼成
//! 多行 INSERT。写盘流式进行，不在内存中保留整库。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use sqlx::MySqlPool;

use super::explorer as explorer_adapter;
use super::query as query_adapter;
use super::quote_identifier;
use super::schema as schema_adapter;
use super::table::column_metadata;
use crate::errors::{error_code, AppError};
use crate::models::database::TableKind;
use crate::models::export::{DumpDatabaseResponse, DumpMode};
use crate::models::query::{DatabaseValue, QueryColumn};

/// 导出整库（或指定表）为 SQL 脚本。
pub async fn dump_database(
    pool: &MySqlPool,
    database: &str,
    tables: Option<&[String]>,
    mode: DumpMode,
    path: &str,
) -> Result<DumpDatabaseResponse, AppError> {
    let started = Instant::now();

    if path.trim().is_empty() {
        return Err(AppError::validation("导出路径不能为空"));
    }

    // 确定要导出的表清单（显式指定或全部物理表）
    let table_list: Vec<String> = match tables {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => explorer_adapter::list_tables(pool, database)
            .await?
            .into_iter()
            .filter(|item| item.table_kind == TableKind::Table)
            .map(|item| item.name)
            .collect(),
    };

    let file = File::create(path)
        .map_err(|error| AppError::storage(format!("无法创建导出文件：{error}")))?;
    let mut writer = BufWriter::with_capacity(128 * 1024, file);

    let mode_label = match mode {
        DumpMode::Structure => "仅结构",
        DumpMode::Data => "仅数据",
        DumpMode::Both => "结构+数据",
    };
    writeln!(
        writer,
        "-- DataDock 数据库导出：{database}（模式：{mode_label}）"
    )
    .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
    writeln!(
        writer,
        "-- 导出时间（Unix 毫秒）：{}",
        crate::utils::time::now_ms()
    )
    .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
    writeln!(writer).map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;

    let mut rows_written: u64 = 0;

    for table in &table_list {
        let table_ref = format!(
            "{}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );

        writeln!(writer, "-- ----------------------------")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
        writeln!(writer, "-- {table}")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
        writeln!(writer, "-- ----------------------------")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;

        if mode.includes_structure() {
            let ddl = schema_adapter::table_create_sql(pool, database, table).await?;
            writeln!(writer, "DROP TABLE IF EXISTS {table_ref};")
                .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
            writeln!(writer, "{ddl};")
                .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
            writeln!(writer)
                .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
        }

        if mode.includes_data() {
            rows_written += dump_table_data(&mut writer, pool, &table_ref, database, table).await?;
            writeln!(writer)
                .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
        }
    }

    writer
        .flush()
        .map_err(|error| AppError::storage(format!("保存导出文件失败：{error}")))?;

    tracing::info!(
        target: "datadock::dump",
        database,
        tables = table_list.len(),
        rows_written,
        "数据库导出完成"
    );

    Ok(DumpDatabaseResponse {
        path: path.to_string(),
        tables: table_list.len() as u32,
        rows_written,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// 导出单表数据为 INSERT 语句，返回写入行数。
async fn dump_table_data(
    writer: &mut BufWriter<File>,
    pool: &MySqlPool,
    table_ref: &str,
    database: &str,
    table: &str,
) -> Result<u64, AppError> {
    let metadata = column_metadata(pool, database, table).await?;
    let columns: Vec<QueryColumn> = metadata
        .iter()
        .map(|column| QueryColumn {
            name: column.name.clone(),
            type_name: column.column_type.clone(),
        })
        .collect();

    if columns.is_empty() {
        return Ok(0);
    }

    let column_list = columns
        .iter()
        .map(|column| quote_identifier(&column.name))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");

    writeln!(writer, "-- 表数据：{table}")
        .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;

    let mut rows_written: u64 = 0;
    let mut offset: u64 = 0;
    const BATCH: u64 = 500;

    loop {
        let sql = format!("SELECT * FROM {table_ref} LIMIT {BATCH} OFFSET {offset}");

        let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("读取导出数据失败：{error}"),
            )
        })?;

        if rows.is_empty() {
            break;
        }

        for row in &rows {
            let cells = query_adapter::convert_row(row, &columns);
            let values = cells
                .iter()
                .map(format_sql_value)
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(
                writer,
                "INSERT INTO {table_ref} ({column_list}) VALUES ({values});"
            )
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
            rows_written += 1;
        }

        if (rows.len() as u64) < BATCH {
            break;
        }
        offset += rows.len() as u64;
    }

    Ok(rows_written)
}

/// 把数据库值转为 SQL 字面量（NULL / 数字 / 字符串）。
fn format_sql_value(value: &DatabaseValue) -> String {
    let Some(raw) = value.value.as_deref() else {
        return "NULL".to_string();
    };

    match value.value_type.as_str() {
        "integer" | "decimal" | "float" | "boolean" => raw.to_string(),
        "binary" => "NULL".to_string(), // 二进制只拿到占位，无法还原原始字节
        _ => {
            let escaped = raw
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            format!("'{escaped}'")
        }
    }
}
