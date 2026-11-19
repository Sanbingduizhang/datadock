//! 表数据 / 结果集导出的写盘工具（CSV / JSON）。
//!
//! 由各引擎适配器（MySQL / PostgreSQL）共用：写盘语义一致，
//! 只有「如何取数」因引擎而异。CSV 带 UTF-8 BOM 并遵循 RFC 4180，
//! JSON 输出为对象数组（值为字符串，NULL 为 null）。

use std::io::Write;

use crate::errors::AppError;
use crate::models::export::ExportFormat;
use crate::models::query::{DatabaseValue, QueryColumn};

pub fn write_header<W: Write>(
    writer: &mut W,
    columns: &[QueryColumn],
    format: ExportFormat,
) -> Result<(), AppError> {
    if format != ExportFormat::Csv {
        return Ok(());
    }

    // UTF-8 BOM：让 Excel 直接识别中文表头
    writer
        .write_all(b"\xEF\xBB\xBF")
        .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;

    let header: Vec<String> = columns
        .iter()
        .map(|column| escape_csv(&column.name))
        .collect();

    writeln!(writer, "{}", header.join(","))
        .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))
}

pub fn write_csv_record<W: Write>(writer: &mut W, cells: &[DatabaseValue]) -> Result<(), AppError> {
    let line: Vec<String> = cells
        .iter()
        .map(|cell| match &cell.value {
            Some(value) => escape_csv(value),
            None => String::new(),
        })
        .collect();

    writeln!(writer, "{}", line.join(","))
        .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))
}

pub fn write_json_record<W: Write>(
    writer: &mut W,
    columns: &[QueryColumn],
    cells: &[DatabaseValue],
    first: bool,
) -> Result<(), AppError> {
    if !first {
        writer
            .write_all(b",\n")
            .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))?;
    }

    let mut record = String::from("  {");
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            record.push(',');
        }

        let name = serde_json::to_string(&column.name)
            .map_err(|error| AppError::internal(format!("序列化列名失败：{error}")))?;
        record.push_str(&name);
        record.push(':');

        match cells.get(index).and_then(|cell| cell.value.as_deref()) {
            Some(value) => {
                let text = serde_json::to_string(value)
                    .map_err(|error| AppError::internal(format!("序列化单元格失败：{error}")))?;
                record.push_str(&text);
            }
            None => record.push_str("null"),
        }
    }
    record.push('}');

    writer
        .write_all(record.as_bytes())
        .map_err(|error| AppError::storage(format!("写入导出文件失败：{error}")))
}

/// RFC 4180：包含引号、逗号或换行的字段用引号包裹，内部引号翻倍
fn escape_csv(value: &str) -> String {
    if value.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapes_special_characters() {
        assert_eq!(escape_csv("plain"), "plain");
        assert_eq!(escape_csv("a,b"), "\"a,b\"");
        assert_eq!(escape_csv("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(escape_csv("line\nbreak"), "\"line\nbreak\"");
    }
}
