//! 连接管理 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::connection::{
    ConnectDatabaseResponse, ConnectionExportFile, ConnectionIdRequest, ConnectionImportResponse,
    ConnectionInfo, ConnectionRuntimeInfo, CreateConnectionRequest, CreateConnectionResponse,
    DisconnectDatabaseResponse, SimpleSuccessResponse, TestConnectionRequest,
    TestConnectionResponse, UpdateConnectionRequest,
};
use crate::state::AppState;

#[tauri::command]
pub async fn list_connections(state: State<'_, AppState>) -> Result<Vec<ConnectionInfo>, AppError> {
    state.connections.list().await
}

#[tauri::command]
pub async fn create_connection(
    request: CreateConnectionRequest,
    state: State<'_, AppState>,
) -> Result<CreateConnectionResponse, AppError> {
    state.connections.create(request).await
}

#[tauri::command]
pub async fn update_connection(
    request: UpdateConnectionRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.connections.update(request).await
}

#[tauri::command]
pub async fn delete_connection(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.connections.delete(&request.connection_id).await
}

#[tauri::command]
pub async fn test_connection(
    request: TestConnectionRequest,
    state: State<'_, AppState>,
) -> Result<TestConnectionResponse, AppError> {
    state.connections.test(request).await
}

#[tauri::command]
pub async fn connect_database(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<ConnectDatabaseResponse, AppError> {
    state.connections.connect(&request.connection_id).await
}

#[tauri::command]
pub async fn disconnect_database(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<DisconnectDatabaseResponse, AppError> {
    state.connections.disconnect(&request.connection_id).await
}

#[tauri::command]
pub async fn get_connection_status(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<ConnectionRuntimeInfo, AppError> {
    Ok(state.connections.status(&request.connection_id).await)
}

/// 导出全部连接配置（JSON，不含密码）。
#[tauri::command]
pub async fn export_connections(
    state: State<'_, AppState>,
) -> Result<ConnectionExportFile, AppError> {
    state.connections.export_connections().await
}

/// 导入连接配置：JSON 文本解析后按名称去重导入，返回导入与跳过数量。
#[tauri::command]
pub async fn import_connections(
    json: String,
    state: State<'_, AppState>,
) -> Result<ConnectionImportResponse, AppError> {
    let file = serde_json::from_str::<ConnectionExportFile>(&json).map_err(|_| {
        AppError::validation("连接导入文件格式无效，请选择 DataDock 导出的连接 JSON")
    })?;
    state.connections.import_connections(&file).await
}
