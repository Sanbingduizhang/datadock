//! SQL 收藏 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::favorite::{
    DeleteSqlFavoriteRequest, NewSqlFavorite, SqlFavorite, UpdateSqlFavoriteRequest,
};
use crate::state::AppState;

#[tauri::command]
pub async fn list_sql_favorites(state: State<'_, AppState>) -> Result<Vec<SqlFavorite>, AppError> {
    state.favorites.list().await
}

#[tauri::command]
pub async fn create_sql_favorite(
    request: NewSqlFavorite,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    state.favorites.create(request).await
}

#[tauri::command]
pub async fn update_sql_favorite(
    request: UpdateSqlFavoriteRequest,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state
        .favorites
        .update(&request.favorite_id, request.favorite)
        .await
}

#[tauri::command]
pub async fn delete_sql_favorite(
    request: DeleteSqlFavoriteRequest,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    state.favorites.delete(&request.favorite_id).await
}
