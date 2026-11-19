//! 应用基础 Command。
//!
//! Command 层只负责：接收参数 → 调用 Service → 返回结果。
//! 禁止在此层直接操作 SQLite、数据库连接或拼接业务 SQL。

use tauri::{AppHandle, State};

use crate::errors::AppError;
use crate::models::app::{AppInfo, PingRequest, PingResponse};
use crate::state::AppState;

/// IPC 连通性检查
#[tauri::command]
pub async fn ping(
    request: PingRequest,
    state: State<'_, AppState>,
) -> Result<PingResponse, AppError> {
    tracing::debug!(target: "datadock::ipc", "收到 ping 请求");
    state.app.ping(request)
}

/// 获取应用与运行环境信息
#[tauri::command]
pub fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    tracing::debug!(target: "datadock::ipc", "收到 get_app_info 请求");
    let package = app.package_info();
    state
        .app
        .app_info(&package.name, &package.version.to_string())
}
