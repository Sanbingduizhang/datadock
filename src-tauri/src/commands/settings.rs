//! 应用设置 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::settings::{AppSettings, UpdateSettingsRequest, UpdateSettingsResponse};
use crate::state::AppState;

/// 获取全部应用设置
#[tauri::command]
pub async fn get_all_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    state.settings.load().await
}

/// 部分更新应用设置（后端二次校验）
#[tauri::command]
pub async fn update_settings(
    request: UpdateSettingsRequest,
    state: State<'_, AppState>,
) -> Result<UpdateSettingsResponse, AppError> {
    state.settings.update(&request.settings).await?;
    Ok(UpdateSettingsResponse { success: true })
}
