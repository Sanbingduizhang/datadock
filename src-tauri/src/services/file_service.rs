//! SQL 文件读写业务。
//!
//! 只做纯文本读写：编码按 UTF-8 解析，超过上限或编码不合法时给出明确错误。

use std::path::Path;

use crate::errors::AppError;
use crate::models::file::{ReadSqlFileResponse, WriteSqlFileResponse, MAX_SQL_FILE_BYTES};

pub fn read_sql_file(path: &str) -> Result<ReadSqlFileResponse, AppError> {
    if path.trim().is_empty() {
        return Err(AppError::validation("文件路径不能为空"));
    }

    let file_path = Path::new(path);
    let metadata = std::fs::metadata(file_path)
        .map_err(|error| AppError::storage(format!("无法读取文件：{error}")))?;

    if metadata.len() > MAX_SQL_FILE_BYTES {
        return Err(AppError::validation(format!(
            "文件过大（{} MB），单文件上限为 {} MB",
            metadata.len() / 1024 / 1024,
            MAX_SQL_FILE_BYTES / 1024 / 1024
        )));
    }

    let bytes = std::fs::read(file_path)
        .map_err(|error| AppError::storage(format!("无法读取文件：{error}")))?;

    let content = String::from_utf8(bytes)
        .map_err(|_| AppError::validation("文件不是 UTF-8 编码，暂不支持该编码的 SQL 文件"))?;

    Ok(ReadSqlFileResponse {
        path: path.to_string(),
        name: file_name(file_path),
        content,
    })
}

pub fn write_sql_file(path: &str, content: &str) -> Result<WriteSqlFileResponse, AppError> {
    if path.trim().is_empty() {
        return Err(AppError::validation("文件路径不能为空"));
    }
    if content.len() as u64 > MAX_SQL_FILE_BYTES {
        return Err(AppError::validation("内容过大，无法保存为 SQL 文件"));
    }

    let file_path = Path::new(path);
    std::fs::write(file_path, content.as_bytes())
        .map_err(|error| AppError::storage(format!("保存文件失败：{error}")))?;

    Ok(WriteSqlFileResponse {
        path: path.to_string(),
        name: file_name(file_path),
        bytes: content.len() as u64,
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trip() {
        let path = std::env::temp_dir().join(format!("datadock-sql-{}.sql", std::process::id()));
        let path_str = path.to_string_lossy().to_string();

        let written = write_sql_file(&path_str, "SELECT 1;").expect("写入应成功");
        assert_eq!(written.bytes, 9);

        let read = read_sql_file(&path_str).expect("读取应成功");
        assert_eq!(read.content, "SELECT 1;");
        assert!(read.name.ends_with(".sql"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_missing_file() {
        assert!(read_sql_file("/definitely/not/here.sql").is_err());
    }

    #[test]
    fn rejects_empty_path() {
        assert!(write_sql_file("  ", "SELECT 1").is_err());
    }
}
