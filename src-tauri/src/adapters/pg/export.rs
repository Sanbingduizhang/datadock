//! PostgreSQL 表数据导出（CSV / JSON）。
//!
//! 分批读取（`LIMIT/OFFSET`，顺序与应用内浏览一致），边读边写，
//! 不在内存中保留整表，也不把数据经 IPC 传回前端。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use sqlx::PgPool;

use super::super::export_writer::{write_csv_record, write_header, write_json_record};
use super::query as pg_query;
use super::schema::quote_identifier;
use super::table::column_metadata;
use crate::errors::{error_code, AppError};
use crate::models::export::{ExportFormat, ExportResponse, EXPORT_BATCH_SIZE, MAX_EXPORT_ROWS};
use crate::models::query::QueryColumn;

#[allow(clippy::too_many_arguments)]
pub async fn export_table(
    pool: &PgPool,
    schema: &str,
    table: &str,
    format: ExportFormat,
    path: &str,
    order_by: Option<&str>,
    order_desc: bool,
    filter: Option<&str>,
) -> Result<ExportResponse, AppError> {
    let started = Instant::now();

    if path.trim().is_empty() {
        return Err(AppError::validation("导出路径不能为空"));
    }

    let table_ref = format!("{}.{}", quote_identifier(schema)?, quote_identifier(table)?);

    let metadata = column_metadata(pool, schema, table).await?;
    if let Some(column) = order_by {
        if !metadata
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(column))
        {
            return Err(AppError::validation(format!("排序列 {column} 不存在")));
        }
    }

    let primary_key: Vec<&str> = metadata
        .iter()
        .filter(|column| column.primary)
        .map(|column| column.name.as_str())
        .collect();

    let where_clause = match filter {
        Some(filter) => format!(" WHERE {filter}"),
        None => String::new(),
    };
    let sort_column = order_by.or_else(|| primary_key.first().copied());
    let order_clause = match sort_column {
        Some(column) => format!(
            " ORDER BY {}{}",
            quote_identifier(column)?,
            if order_desc { " DESC" } else { " ASC" }
        ),
        None => String::new(),
    };

    let file = File::create(path)
        .map_err(|error| AppError::storage(format!("无法创建导出文件：{error}")))?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    let mut columns: Vec<QueryColumn> = metadata
        .iter()
        .map(|column| QueryColumn {
            name: column.name.clone(),
            type_name: column.column_type.clone(),
        })
        .collect();

    let mut header_written = false;
    if !columns.is_empty() {
        write_header(&mut writer, &columns, format)?;
        header_written = true;
    }

    if format == ExportFormat::Json {
        writer
            .write_all(b"[\n")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
    }

    let mut rows_written: u64 = 0;
    let mut offset: u64 = 0;
    let mut truncated = false;
    let mut first_record = true;

    loop {
        let sql = format!(
            "SELECT * FROM {table_ref}{where_clause}{order_clause} LIMIT {EXPORT_BATCH_SIZE} OFFSET {offset}"
        );

        let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("读取导出数据失败：{error}"),
            )
        })?;

        if rows.is_empty() {
            break;
        }

        if !header_written || columns.is_empty() {
            if columns.is_empty() {
                columns = pg_query::columns_of(&rows[0]);
            }
            write_header(&mut writer, &columns, format)?;
            header_written = true;
        }

        for row in &rows {
            let cells = pg_query::convert_row(row, &columns);

            match format {
                ExportFormat::Csv => write_csv_record(&mut writer, &cells)?,
                ExportFormat::Json => {
                    write_json_record(&mut writer, &columns, &cells, first_record)?;
                    first_record = false;
                }
            }

            rows_written += 1;
            if rows_written >= MAX_EXPORT_ROWS {
                truncated = true;
                break;
            }
        }

        if truncated || (rows.len() as u32) < EXPORT_BATCH_SIZE {
            break;
        }

        offset += rows.len() as u64;
    }

    if format == ExportFormat::Json {
        writer
            .write_all(b"\n]\n")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
    }

    writer
        .flush()
        .map_err(|error| AppError::storage(format!("保存导出文件失败：{error}")))?;

    tracing::info!(
        target: "datadock::export",
        schema,
        table,
        rows = rows_written,
        truncated,
        "导出完成"
    );

    Ok(ExportResponse {
        path: path.to_string(),
        rows_written,
        elapsed_ms: started.elapsed().as_millis() as u64,
        truncated,
    })
}
