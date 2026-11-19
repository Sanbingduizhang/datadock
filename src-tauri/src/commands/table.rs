//! 表数据 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::export::{ExportResponse, ExportTableRequest};
use crate::models::table::{TableDataRequest, TableDataResponse};
use crate::state::AppState;

#[tauri::command]
pub async fn get_table_data(
    request: TableDataRequest,
    state: State<'_, AppState>,
) -> Result<TableDataResponse, AppError> {
    state.table.data(request).await
}

#[tauri::command]
pub async fn export_table_data(
    request: ExportTableRequest,
    state: State<'_, AppState>,
) -> Result<ExportResponse, AppError> {
    state.table.export(request).await
}
