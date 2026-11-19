//! 数据库资源查询 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::connection::ConnectionIdRequest;
use crate::models::database::{DatabaseInfo, ListTablesRequest, TableInfo};
use crate::state::AppState;

#[tauri::command]
pub async fn list_databases(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<Vec<DatabaseInfo>, AppError> {
    state.explorer.list_databases(&request.connection_id).await
}

#[tauri::command]
pub async fn list_tables(
    request: ListTablesRequest,
    state: State<'_, AppState>,
) -> Result<Vec<TableInfo>, AppError> {
    state
        .explorer
        .list_tables(&request.connection_id, &request.database_name)
        .await
}
