//! Redis Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::connection::{ConnectionIdRequest, SimpleSuccessResponse};
use crate::models::database::DatabaseInfo;
use crate::models::query::ExecuteSqlResponse;
use crate::models::redis::{
    RedisDeleteKeyRequest, RedisKeyValueRequest, RedisKeyValueResponse, RedisRenameKeyRequest,
    RedisScanRequest, RedisScanResponse, RedisSetKeyValueRequest, RedisSetTtlRequest,
};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisExecuteRequest {
    pub connection_id: String,
    pub command: String,
}

#[tauri::command]
pub async fn redis_execute(
    request: RedisExecuteRequest,
    state: State<'_, AppState>,
) -> Result<ExecuteSqlResponse, AppError> {
    state
        .redis
        .execute(&request.connection_id, &request.command)
        .await
}

#[tauri::command]
pub async fn redis_list_databases(
    request: ConnectionIdRequest,
    state: State<'_, AppState>,
) -> Result<Vec<DatabaseInfo>, AppError> {
    state.redis.list_databases(&request.connection_id).await
}

#[tauri::command]
pub async fn redis_scan_keys(
    request: RedisScanRequest,
    state: State<'_, AppState>,
) -> Result<RedisScanResponse, AppError> {
    state.redis.scan_keys(request).await
}

#[tauri::command]
pub async fn redis_get_key_value(
    request: RedisKeyValueRequest,
    state: State<'_, AppState>,
) -> Result<RedisKeyValueResponse, AppError> {
    state.redis.get_key_value(request).await
}

#[tauri::command]
pub async fn redis_delete_key(
    request: RedisDeleteKeyRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.redis.delete_key(request).await
}

#[tauri::command]
pub async fn redis_rename_key(
    request: RedisRenameKeyRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.redis.rename_key(request).await
}

#[tauri::command]
pub async fn redis_set_ttl(
    request: RedisSetTtlRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.redis.set_ttl(request).await
}

#[tauri::command]
pub async fn redis_set_key_value(
    request: RedisSetKeyValueRequest,
    state: State<'_, AppState>,
) -> Result<SimpleSuccessResponse, AppError> {
    state.redis.set_key_value(request).await
}
