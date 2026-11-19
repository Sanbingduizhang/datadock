//! 工作区状态 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::workspace::{
    DeleteWorkspaceRequest, GetWorkspaceRequest, GetWorkspaceResponse, SaveWorkspaceRequest,
    SaveWorkspaceResponse, WorkspaceSummary,
};
use crate::state::AppState;

/// 读取工作区状态；首次启动返回 state = null
#[tauri::command]
pub async fn get_workspace(
    request: GetWorkspaceRequest,
    state: State<'_, AppState>,
) -> Result<GetWorkspaceResponse, AppError> {
    let workspace_id = request.workspace_id;
    let saved = state.workspace.load(&workspace_id).await?;

    Ok(GetWorkspaceResponse {
        state: saved,
        workspace_id,
    })
}

/// 保存工作区状态
#[tauri::command]
pub async fn save_workspace(
    request: SaveWorkspaceRequest,
    state: State<'_, AppState>,
) -> Result<SaveWorkspaceResponse, AppError> {
    state
        .workspace
        .save(&request.workspace_id, &request.state)
        .await?;

    Ok(SaveWorkspaceResponse { success: true })
}

#[tauri::command]
pub async fn list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceSummary>, AppError> {
    state.workspace.list().await
}

#[tauri::command]
pub async fn delete_workspace(
    request: DeleteWorkspaceRequest,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state.workspace.delete(&request.workspace_id).await
}
