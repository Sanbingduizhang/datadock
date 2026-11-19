//! 查询历史 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::history::{
    ClearQueryHistoryRequest, DeleteQueryHistoryRequest, ListQueryHistoryRequest,
    ListQueryHistoryResponse,
};
use crate::state::AppState;

#[tauri::command]
pub async fn list_query_history(
    request: ListQueryHistoryRequest,
    state: State<'_, AppState>,
) -> Result<ListQueryHistoryResponse, AppError> {
    state.history.list(request).await
}

#[tauri::command]
pub async fn delete_query_history(
    request: DeleteQueryHistoryRequest,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state.history.delete(&request.history_id).await
}

#[tauri::command]
pub async fn clear_query_history(
    request: ClearQueryHistoryRequest,
    state: State<'_, AppState>,
) -> Result<u64, AppError> {
    state.history.clear(request.connection_id.as_deref()).await
}
