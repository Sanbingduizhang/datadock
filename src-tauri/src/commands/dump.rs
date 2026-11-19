//! 数据库导入导出 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::export::{
    DumpDatabaseRequest, DumpDatabaseResponse, ImportDatabaseRequest, ImportDatabaseResponse,
};
use crate::state::AppState;

#[tauri::command]
pub async fn dump_database(
    request: DumpDatabaseRequest,
    state: State<'_, AppState>,
) -> Result<DumpDatabaseResponse, AppError> {
    state.dump.dump(request).await
}

#[tauri::command]
pub async fn import_database(
    request: ImportDatabaseRequest,
    state: State<'_, AppState>,
) -> Result<ImportDatabaseResponse, AppError> {
    state.dump.import(request).await
}
