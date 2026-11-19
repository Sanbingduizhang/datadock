//! SQL 文件读写 Command。

use crate::errors::AppError;
use crate::models::file::{
    ReadSqlFileRequest, ReadSqlFileResponse, WriteSqlFileRequest, WriteSqlFileResponse,
};
use crate::services::file_service;

#[tauri::command]
pub async fn read_sql_file(request: ReadSqlFileRequest) -> Result<ReadSqlFileResponse, AppError> {
    file_service::read_sql_file(&request.path)
}

#[tauri::command]
pub async fn write_sql_file(
    request: WriteSqlFileRequest,
) -> Result<WriteSqlFileResponse, AppError> {
    file_service::write_sql_file(&request.path, &request.content)
}
