//! SQL 执行与取消 Command。

use tauri::State;

use crate::errors::AppError;
use crate::models::export::{ExportQueryRequest, ExportResponse};
use crate::models::query::{
    BeginTransactionRequest, BeginTransactionResponse, CancelTaskRequest, CancelTaskResponse,
    ExecuteBatchResponse, ExecuteSqlRequest, ExecuteSqlResponse, TransactionRequest,
    TransactionResponse,
};
use crate::state::AppState;

#[tauri::command]
pub async fn execute_sql(
    request: ExecuteSqlRequest,
    state: State<'_, AppState>,
) -> Result<ExecuteSqlResponse, AppError> {
    state.query.execute(request).await
}

/// 批量执行：整体事务包裹，失败自动回滚
#[tauri::command]
pub async fn execute_batch_sql(
    request: ExecuteSqlRequest,
    state: State<'_, AppState>,
) -> Result<ExecuteBatchResponse, AppError> {
    state.query.execute_batch(request).await
}

/// 取消正在执行的查询（服务端 `KILL QUERY`）
#[tauri::command]
pub async fn cancel_task(
    request: CancelTaskRequest,
    state: State<'_, AppState>,
) -> Result<CancelTaskResponse, AppError> {
    state.query.cancel(&request.task_id).await
}

/// 导出只读查询的结果到本地文件
#[tauri::command]
pub async fn export_query_result(
    request: ExportQueryRequest,
    state: State<'_, AppState>,
) -> Result<ExportResponse, AppError> {
    state.query.export_result(request).await
}

#[tauri::command]
pub async fn begin_transaction(
    request: BeginTransactionRequest,
    state: State<'_, AppState>,
) -> Result<BeginTransactionResponse, AppError> {
    state.query.begin_transaction(request).await
}

#[tauri::command]
pub async fn commit_transaction(
    request: TransactionRequest,
    state: State<'_, AppState>,
) -> Result<TransactionResponse, AppError> {
    state.query.commit_transaction(request).await
}

#[tauri::command]
pub async fn rollback_transaction(
    request: TransactionRequest,
    state: State<'_, AppState>,
) -> Result<TransactionResponse, AppError> {
    state.query.rollback_transaction(request).await
}
