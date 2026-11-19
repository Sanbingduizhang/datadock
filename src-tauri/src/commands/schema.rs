//! 表结构查询 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::database::{DatabaseDdl, DatabaseDdlRequest, TableSchema, TableSchemaRequest};
use crate::state::AppState;

#[tauri::command]
pub async fn get_table_schema(
    request: TableSchemaRequest,
    state: State<'_, AppState>,
) -> Result<TableSchema, AppError> {
    state
        .schema
        .table_schema(
            &request.connection_id,
            &request.database_name,
            &request.table_name,
        )
        .await
}

#[tauri::command]
pub async fn get_database_ddl(
    request: DatabaseDdlRequest,
    state: State<'_, AppState>,
) -> Result<DatabaseDdl, AppError> {
    state
        .schema
        .database_ddl(&request.connection_id, &request.database_name)
        .await
}
