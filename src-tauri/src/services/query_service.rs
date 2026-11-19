//! SQL 执行业务：多语句顺序执行、任务取消、查询历史记录。
//!
//! 执行会话使用独立的单连接池：
//! - `SET SESSION`（结果行数上限）只会作用在真正执行 SQL 的连接上；
//! - 可以准确拿到 `CONNECTION_ID()`，取消时用另一条连接执行 `KILL QUERY`。
//!
//! 取消采用「客户端生成 taskId → 同步等待结果 + 并发 cancel_task」模型，
//! 不引入任务队列与事件轮询（见 TDR-003）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

use crate::adapters::mysql::query as mysql_query;
use crate::core::ConnectionManager;
use crate::database::Database;
use crate::errors::{error_code, AppError};
use crate::models::export::{ExportQueryRequest, ExportResponse};
use crate::models::history::{NewQueryHistory, QueryHistoryOutcome, QueryHistoryStatus};
use crate::models::query::{
    preview, BeginTransactionRequest, BeginTransactionResponse, CancelTaskResponse,
    ExecuteBatchResponse, ExecuteSqlRequest, ExecuteSqlResponse, StatementError, StatementResult,
    TransactionRequest, TransactionResponse, DEFAULT_HISTORY_LIMIT, MAX_STATEMENTS,
};
use crate::models::settings::{KEY_HISTORY_ENABLED, KEY_HISTORY_LIMIT};
use crate::repositories::{history_repository, settings_repository};
use crate::utils::sql::split_statements;
use crate::utils::time::{new_id, now_ms, to_db_time};

/// 每执行多少次清理一次历史，避免每次查询都做删除
const PRUNE_INTERVAL: u64 = 20;

/// 正在执行的查询，用于取消
struct ActiveQuery {
    connection_id: String,
    session_id: u64,
    cancelled: Arc<AtomicBool>,
}

pub struct QueryService {
    manager: Arc<ConnectionManager>,
    pg_manager: Arc<crate::core::PgManager>,
    database: Database,
    active: Mutex<HashMap<String, Arc<ActiveQuery>>>,
    executed: AtomicU64,
}

impl QueryService {
    pub fn new(
        manager: Arc<ConnectionManager>,
        pg_manager: Arc<crate::core::PgManager>,
        database: Database,
    ) -> Self {
        Self {
            manager,
            pg_manager,
            database,
            active: Mutex::new(HashMap::new()),
            executed: AtomicU64::new(0),
        }
    }

    /// PostgreSQL 执行路径：普通语句复用长连接池；事务内语句复用独立单连接会话。
    /// 不支持取消（无 KILL QUERY 等价）。
    async fn execute_pg(
        &self,
        pg: Arc<crate::core::ManagedPg>,
        request: &ExecuteSqlRequest,
    ) -> Result<ExecuteSqlResponse, AppError> {
        use crate::adapters::pg::query as pg_query;
        use crate::utils::sql::split_statements;

        let started = Instant::now();
        let max_rows = request.normalized_max_rows();
        let statements = split_statements(&request.sql);
        if statements.is_empty() {
            return Err(AppError::new(error_code::SQL_EMPTY, "SQL 内容为空"));
        }

        // 事务内语句必须复用同一个会话；普通执行复用长连接池
        let transaction_pool = match request.transaction_id.as_deref() {
            Some(transaction_id) => {
                Some(pg.transaction_pool(transaction_id).await.ok_or_else(|| {
                    AppError::new(
                        error_code::CONNECTION_NOT_READY,
                        "事务会话不存在，可能已提交或回滚",
                    )
                })?)
            }
            None => None,
        };

        let mut results: Vec<StatementResult> = Vec::with_capacity(statements.len());
        let mut failure: Option<StatementError> = None;
        let mut first_error: Option<crate::errors::AppError> = None;

        for (offset, sql) in statements.iter().enumerate() {
            let statement_started = Instant::now();
            let sql_preview = preview(sql);

            // 事务内用独立会话，普通执行用长连接池
            let pool: &sqlx::PgPool = match transaction_pool.as_ref() {
                Some(pool) => pool,
                None => &pg.pool,
            };

            match pg_query::execute_one(pool, sql, max_rows).await {
                Ok(outcome) => {
                    results.push(StatementResult {
                        index: (offset + 1) as u32,
                        sql_preview,
                        result_set: outcome.result_set,
                        affected_rows: outcome.affected_rows,
                        last_insert_id: None,
                        elapsed_ms: statement_started.elapsed().as_millis() as u64,
                        message: outcome.message,
                    });
                }
                Err(error) => {
                    failure = Some(StatementError {
                        index: (offset + 1) as u32,
                        sql_preview,
                        message: error.message.clone(),
                    });
                    first_error = Some(error);
                    break;
                }
            }
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        let message = match (&failure, &first_error) {
            (Some(error), _) => format!("第 {} 条语句失败：{}", error.index, error.message),
            (None, _) => format!("已执行 {} 条语句", results.len()),
        };

        Ok(ExecuteSqlResponse {
            task_id: request.task_id.clone().unwrap_or_default(),
            results,
            error: failure,
            elapsed_ms,
            message,
            truncated: false,
            cancelled: false,
            history_id: None,
        })
    }

    pub async fn execute(
        &self,
        request: ExecuteSqlRequest,
    ) -> Result<ExecuteSqlResponse, AppError> {
        // PostgreSQL：复用长连接池，无取消/事务语义
        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            return self.execute_pg(pg, &request).await;
        }

        let statements = split_statements(&request.sql);
        if statements.is_empty() {
            return Err(AppError::new(error_code::SQL_EMPTY, "SQL 内容为空"));
        }
        if statements.len() > MAX_STATEMENTS {
            return Err(AppError::validation(format!(
                "单次最多执行 {MAX_STATEMENTS} 条语句，当前检测到 {} 条",
                statements.len()
            )));
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let started = Instant::now();
        let max_rows = request.normalized_max_rows();
        let task_id = request.task_id.clone().unwrap_or_else(new_id);

        // 事务中的语句必须复用同一个会话；普通执行使用一次性会话
        let manages_pool = request.transaction_id.is_none();
        let pool = match request.transaction_id.as_deref() {
            Some(transaction_id) => connection
                .transaction_pool(transaction_id)
                .await
                .ok_or_else(|| {
                    AppError::new(
                        error_code::CONNECTION_NOT_READY,
                        "事务会话不存在，可能已提交或回滚",
                    )
                })?,
            None => open_session(&connection, request.database_name.as_deref()).await?,
        };

        let session_id = match session_id(&pool).await {
            Ok(session_id) => session_id,
            Err(error) => {
                if manages_pool {
                    pool.close().await;
                }
                return Err(error);
            }
        };

        let cancelled = Arc::new(AtomicBool::new(false));
        self.register(
            &task_id,
            &request.connection_id,
            session_id,
            cancelled.clone(),
        );

        let (history_enabled, history_limit) = self.history_settings().await;
        let history_id = if history_enabled {
            self.start_history(&request).await
        } else {
            None
        };

        let mut results: Vec<StatementResult> = Vec::with_capacity(statements.len());
        let mut failure: Option<StatementError> = None;

        for (offset, sql) in statements.iter().enumerate() {
            // 在语句之间取消：直接停止，不再执行后续语句
            if cancelled.load(Ordering::SeqCst) {
                break;
            }

            let statement_started = Instant::now();
            let sql_preview = preview(sql);

            match mysql_query::execute_one(&pool, sql, max_rows).await {
                Ok(outcome) => {
                    // 执行期间被取消：该语句的返回值不可信（例如被中断的 SLEEP 返回 1），
                    // 因此丢弃这一条的结果，只保留取消前已完成的语句。
                    if cancelled.load(Ordering::SeqCst) {
                        tracing::info!(
                            target: "datadock::query",
                            task_id,
                            statement = offset + 1,
                            "语句执行期间收到取消，丢弃该语句结果"
                        );
                        break;
                    }

                    results.push(StatementResult {
                        index: (offset + 1) as u32,
                        sql_preview,
                        result_set: outcome.result_set,
                        affected_rows: outcome.affected_rows,
                        last_insert_id: outcome.last_insert_id,
                        elapsed_ms: statement_started.elapsed().as_millis() as u64,
                        message: outcome.message,
                    });
                }
                Err(error) => {
                    failure = Some(StatementError {
                        index: (offset + 1) as u32,
                        sql_preview,
                        message: error.message.clone(),
                    });
                    break;
                }
            }
        }

        self.unregister(&task_id);
        if manages_pool {
            pool.close().await;
        }

        let was_cancelled = cancelled.load(Ordering::SeqCst);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let truncated = results
            .iter()
            .filter_map(|item| item.result_set.as_ref())
            .any(|set| set.truncated);

        let total_statements = statements.len() as u32;
        let message = if was_cancelled {
            format!(
                "已取消执行（已完成 {} / {} 条语句）",
                results.len(),
                total_statements
            )
        } else if let Some(error) = &failure {
            format!("第 {} 条语句执行失败：{}", error.index, error.message)
        } else if results.len() == 1 {
            results
                .first()
                .map(|item| item.message.clone())
                .unwrap_or_default()
        } else {
            summarize(&results)
        };

        let outcome = QueryHistoryOutcome {
            status: if was_cancelled {
                QueryHistoryStatus::Cancelled
            } else if failure.is_some() {
                QueryHistoryStatus::Failed
            } else {
                QueryHistoryStatus::Success
            },
            returned_rows: total_returned_rows(&results),
            affected_rows: total_affected_rows(&results),
            elapsed_ms: elapsed_ms as i64,
            error_message: failure.as_ref().map(|error| error.message.clone()),
            finished_at: Some(to_db_time(now_ms())),
        };
        self.finish_history(history_id.as_deref(), &outcome, history_limit)
            .await;

        Ok(ExecuteSqlResponse {
            task_id,
            results,
            error: if was_cancelled { None } else { failure },
            elapsed_ms,
            message,
            truncated,
            cancelled: was_cancelled,
            history_id,
        })
    }

    /// 批量执行：整体包在事务里，任何一条失败则回滚全部。
    ///
    /// 用于表数据的多行变更；DDL 在 MySQL 中会隐式提交，混入 DDL 时无法保证原子性。
    pub async fn execute_batch(
        &self,
        request: ExecuteSqlRequest,
    ) -> Result<ExecuteBatchResponse, AppError> {
        let statements = split_statements(&request.sql);
        if statements.is_empty() {
            return Err(AppError::new(error_code::SQL_EMPTY, "SQL 内容为空"));
        }
        if statements.len() > MAX_STATEMENTS {
            return Err(AppError::validation(format!(
                "单次最多执行 {MAX_STATEMENTS} 条语句，当前检测到 {} 条",
                statements.len()
            )));
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let started = Instant::now();
        let max_rows = request.normalized_max_rows();
        let pool = open_session(&connection, request.database_name.as_deref()).await?;

        if let Err(error) = sqlx::raw_sql("START TRANSACTION").execute(&pool).await {
            pool.close().await;
            return Err(AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("开启事务失败：{error}"),
            ));
        }

        let (history_enabled, history_limit) = self.history_settings().await;
        let history_id = if history_enabled {
            self.start_history(&request).await
        } else {
            None
        };

        let mut results: Vec<StatementResult> = Vec::with_capacity(statements.len());
        let mut failure: Option<StatementError> = None;

        for (offset, sql) in statements.iter().enumerate() {
            let statement_started = Instant::now();
            let sql_preview = preview(sql);

            match mysql_query::execute_one(&pool, sql, max_rows).await {
                Ok(outcome) => results.push(StatementResult {
                    index: (offset + 1) as u32,
                    sql_preview,
                    result_set: outcome.result_set,
                    affected_rows: outcome.affected_rows,
                    last_insert_id: outcome.last_insert_id,
                    elapsed_ms: statement_started.elapsed().as_millis() as u64,
                    message: outcome.message,
                }),
                Err(error) => {
                    failure = Some(StatementError {
                        index: (offset + 1) as u32,
                        sql_preview,
                        message: error.message.clone(),
                    });
                    break;
                }
            }
        }

        let mut rolled_back = false;

        if failure.is_some() {
            if let Err(error) = sqlx::raw_sql("ROLLBACK").execute(&pool).await {
                tracing::warn!(target: "datadock::query", %error, "批量变更回滚失败");
            }
            rolled_back = true;
        } else if let Err(error) = sqlx::raw_sql("COMMIT").execute(&pool).await {
            failure = Some(StatementError {
                index: results.len() as u32,
                sql_preview: "COMMIT".to_string(),
                message: format!("提交失败：{error}"),
            });
            rolled_back = true;
        }

        pool.close().await;

        let elapsed_ms = started.elapsed().as_millis() as u64;
        let message = match (&failure, rolled_back) {
            (Some(error), true) => format!(
                "第 {} 条语句失败，已回滚全部修改：{}",
                error.index, error.message
            ),
            (Some(error), false) => format!("第 {} 条语句失败：{}", error.index, error.message),
            (None, _) => format!("已提交 {} 条语句", results.len()),
        };

        let outcome = QueryHistoryOutcome {
            status: if failure.is_some() {
                QueryHistoryStatus::Failed
            } else {
                QueryHistoryStatus::Success
            },
            returned_rows: total_returned_rows(&results),
            affected_rows: total_affected_rows(&results),
            elapsed_ms: elapsed_ms as i64,
            error_message: failure.as_ref().map(|error| error.message.clone()),
            finished_at: Some(to_db_time(now_ms())),
        };
        self.finish_history(history_id.as_deref(), &outcome, history_limit)
            .await;

        Ok(ExecuteBatchResponse {
            results,
            error: failure,
            rolled_back,
            elapsed_ms,
            message,
            history_id,
        })
    }

    /// 开启手动事务：独立会话执行 `START TRANSACTION`（MySQL）/ `BEGIN`（PG），
    /// 后续语句复用该会话。
    pub async fn begin_transaction(
        &self,
        request: BeginTransactionRequest,
    ) -> Result<BeginTransactionResponse, AppError> {
        // PostgreSQL：从 ManagedPg 参数建立独立单连接池
        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            let params = crate::adapters::pg::connection::PgParams {
                host: pg.host.clone(),
                port: pg.port,
                username: pg.username.clone(),
                password: pg.password.clone(),
                database: Some(pg.database.clone()),
                ssl_mode: pg.ssl_mode,
                connect_timeout: std::time::Duration::from_secs(5),
            };
            let pool = crate::adapters::pg::connection::connect_single(&params).await?;
            if let Err(error) = sqlx::raw_sql("BEGIN").execute(&pool).await {
                pool.close().await;
                return Err(AppError::new(
                    error_code::SQL_EXECUTION_FAILED,
                    format!("开启事务失败：{error}"),
                ));
            }

            let transaction_id = new_id();
            pg.register_transaction(&transaction_id, pool).await;
            tracing::info!(
                target: "datadock::query",
                connection_id = %request.connection_id,
                %transaction_id,
                "PG 事务已开启"
            );
            return Ok(BeginTransactionResponse { transaction_id });
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let pool = open_session(&connection, request.database_name.as_deref()).await?;

        if let Err(error) = sqlx::raw_sql("START TRANSACTION").execute(&pool).await {
            pool.close().await;
            return Err(AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("开启事务失败：{error}"),
            ));
        }

        let transaction_id = new_id();
        connection.register_transaction(&transaction_id, pool).await;

        tracing::info!(
            target: "datadock::query",
            connection_id = %request.connection_id,
            %transaction_id,
            "事务已开启"
        );

        Ok(BeginTransactionResponse { transaction_id })
    }

    pub async fn commit_transaction(
        &self,
        request: TransactionRequest,
    ) -> Result<TransactionResponse, AppError> {
        self.finish_transaction(&request, "COMMIT").await
    }

    pub async fn rollback_transaction(
        &self,
        request: TransactionRequest,
    ) -> Result<TransactionResponse, AppError> {
        self.finish_transaction(&request, "ROLLBACK").await
    }

    async fn finish_transaction(
        &self,
        request: &TransactionRequest,
        statement: &str,
    ) -> Result<TransactionResponse, AppError> {
        // PostgreSQL：从 ManagedPg 取出事务会话
        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            let pool = pg
                .take_transaction(&request.transaction_id)
                .await
                .ok_or_else(|| {
                    AppError::new(
                        error_code::CONNECTION_NOT_READY,
                        "事务会话不存在，可能已提交或回滚",
                    )
                })?;
            let result = sqlx::raw_sql(statement).execute(&pool).await;
            pool.close().await;
            result.map_err(|error| {
                AppError::new(
                    error_code::SQL_EXECUTION_FAILED,
                    format!("{statement} 失败：{error}"),
                )
            })?;
            return Ok(TransactionResponse {
                success: true,
                message: if statement == "COMMIT" {
                    "事务已提交".to_string()
                } else {
                    "事务已回滚".to_string()
                },
            });
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(error_code::CONNECTION_NOT_READY, "连接尚未建立，事务已失效")
            })?;

        let pool = connection
            .take_transaction(&request.transaction_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "事务会话不存在，可能已提交或回滚",
                )
            })?;

        let result = sqlx::raw_sql(statement).execute(&pool).await;
        pool.close().await;

        result.map_err(|error| {
            AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("{statement} 失败：{error}"),
            )
        })?;

        Ok(TransactionResponse {
            success: true,
            message: if statement == "COMMIT" {
                "事务已提交".to_string()
            } else {
                "事务已回滚".to_string()
            },
        })
    }

    /// 导出只读查询的结果。
    ///
    /// 只接受单条返回结果集的语句：导出会重新执行这条 SQL，绝不能用于写操作。
    pub async fn export_result(
        &self,
        request: ExportQueryRequest,
    ) -> Result<ExportResponse, AppError> {
        let statements = split_statements(&request.sql);
        if statements.len() != 1 {
            return Err(AppError::validation("只能导出单条查询语句的结果"));
        }

        let sql = statements
            .first()
            .ok_or_else(|| AppError::new(error_code::SQL_EMPTY, "SQL 内容为空"))?;

        if !mysql_query::returns_rows(sql) {
            return Err(AppError::validation(
                "只能导出会返回结果集的语句（SELECT / SHOW / WITH 等）",
            ));
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let pool = connection
            .pool_for(request.database_name.as_deref())
            .await?;

        crate::adapters::mysql::export::export_query_result(
            &pool,
            sql,
            request.format,
            &request.path,
            request.normalized_max_rows(),
        )
        .await
    }

    /// 取消正在执行的查询：先置标记，再用独立连接执行 `KILL QUERY`。
    pub async fn cancel(&self, task_id: &str) -> Result<CancelTaskResponse, AppError> {
        let active = {
            let guard = self
                .active
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            guard.get(task_id).cloned()
        };

        let Some(active) = active else {
            return Ok(CancelTaskResponse { cancelled: false });
        };

        // 先置标记：即使 KILL 因为时序问题落空，结果也会被标记为已取消
        active.cancelled.store(true, Ordering::SeqCst);

        let Some(connection) = self.manager.get(&active.connection_id).await else {
            return Ok(CancelTaskResponse { cancelled: false });
        };

        // KILL 必须走文本协议，且不能与目标查询共用会话
        let statement = format!("KILL QUERY {}", active.session_id);
        match sqlx::raw_sql(&statement).execute(&connection.pool).await {
            Ok(_) => {
                tracing::info!(
                    target: "datadock::query",
                    task_id,
                    session_id = active.session_id,
                    "已取消查询"
                );
                Ok(CancelTaskResponse { cancelled: true })
            }
            Err(error) => {
                // 查询可能刚好结束，此时服务端会报「Unknown thread id」
                tracing::warn!(
                    target: "datadock::query",
                    task_id,
                    %error,
                    "取消查询未生效"
                );
                Ok(CancelTaskResponse { cancelled: false })
            }
        }
    }

    fn register(
        &self,
        task_id: &str,
        connection_id: &str,
        session_id: u64,
        cancelled: Arc<AtomicBool>,
    ) {
        let mut guard = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        guard.insert(
            task_id.to_string(),
            Arc::new(ActiveQuery {
                connection_id: connection_id.to_string(),
                session_id,
                cancelled,
            }),
        );
    }

    fn unregister(&self, task_id: &str) {
        let mut guard = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        guard.remove(task_id);
    }

    async fn history_settings(&self) -> (bool, u32) {
        let map = match settings_repository::load_map(self.database.pool()).await {
            Ok(map) => map,
            Err(error) => {
                tracing::warn!(target: "datadock::query", %error, "读取历史设置失败，按默认值处理");
                return (true, DEFAULT_HISTORY_LIMIT);
            }
        };

        let enabled = map
            .get(KEY_HISTORY_ENABLED)
            .map(|value| !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let limit = map
            .get(KEY_HISTORY_LIMIT)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_HISTORY_LIMIT)
            .max(1);

        (enabled, limit)
    }

    async fn start_history(&self, request: &ExecuteSqlRequest) -> Option<String> {
        let input = NewQueryHistory {
            connection_id: Some(request.connection_id.clone()),
            source_type: request.source_type().to_string(),
            database_name: request.database_name.clone(),
            sql_text: request.sql.clone(),
        };

        match history_repository::insert(self.database.pool(), &input).await {
            Ok(id) => Some(id),
            Err(error) => {
                tracing::warn!(target: "datadock::query", %error, "写入查询历史失败");
                None
            }
        }
    }

    async fn finish_history(
        &self,
        history_id: Option<&str>,
        outcome: &QueryHistoryOutcome,
        limit: u32,
    ) {
        let Some(history_id) = history_id else {
            return;
        };

        if let Err(error) =
            history_repository::finish(self.database.pool(), history_id, outcome).await
        {
            tracing::warn!(target: "datadock::query", %error, "更新查询历史失败");
            return;
        }

        // 周期性清理，保证历史条数不超过设置上限
        let count = self.executed.fetch_add(1, Ordering::Relaxed);
        if count % PRUNE_INTERVAL == 0 {
            if let Err(error) = history_repository::prune(self.database.pool(), limit).await {
                tracing::warn!(target: "datadock::query", %error, "清理查询历史失败");
            }
        }
    }
}

/// 打开一次性查询会话（单连接，便于 `SET SESSION` 与 `KILL QUERY` 定位）
async fn open_session(
    connection: &crate::core::ManagedConnection,
    database: Option<&str>,
) -> Result<MySqlPool, AppError> {
    MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(15))
        .connect_with(connection.query_options(database))
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                format!("建立查询会话失败：{error}"),
            )
        })
}

async fn session_id(pool: &MySqlPool) -> Result<u64, AppError> {
    sqlx::query_scalar::<_, u64>("SELECT CONNECTION_ID()")
        .fetch_one(pool)
        .await
        .map_err(|error| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                format!("读取会话 ID 失败：{error}"),
            )
        })
}

fn total_returned_rows(results: &[StatementResult]) -> i64 {
    results
        .iter()
        .filter_map(|item| item.result_set.as_ref())
        .map(|set| set.row_count as i64)
        .sum()
}

fn total_affected_rows(results: &[StatementResult]) -> Option<i64> {
    let mut total: i64 = 0;
    let mut any = false;
    for item in results {
        if let Some(affected) = item.affected_rows {
            any = true;
            total += affected as i64;
        }
    }
    any.then_some(total)
}

fn summarize(results: &[StatementResult]) -> String {
    let rows = total_returned_rows(results);
    let affected = total_affected_rows(results);

    let mut parts = Vec::new();
    if rows > 0 {
        parts.push(format!("返回 {rows} 行"));
    }
    if let Some(affected) = affected {
        parts.push(format!("影响 {affected} 行"));
    }
    if parts.is_empty() {
        parts.push("无返回数据".to_string());
    }

    format!("已执行 {} 条语句（{}）", results.len(), parts.join("，"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::query::{QueryColumn, QueryResultSet};

    fn statement(index: u32, rows: usize, affected: Option<u64>) -> StatementResult {
        StatementResult {
            index,
            sql_preview: format!("SELECT {index}"),
            result_set: (affected.is_none()).then(|| QueryResultSet {
                columns: vec![QueryColumn {
                    name: "id".to_string(),
                    type_name: "INT".to_string(),
                }],
                rows: Vec::new(),
                row_count: rows,
                truncated: false,
            }),
            affected_rows: affected,
            last_insert_id: None,
            elapsed_ms: 1,
            message: String::new(),
        }
    }

    #[test]
    fn summary_counts_rows_and_affected() {
        let results = vec![statement(1, 3, None), statement(2, 0, Some(2))];

        assert_eq!(total_returned_rows(&results), 3);
        assert_eq!(total_affected_rows(&results), Some(2));
        assert_eq!(
            summarize(&results),
            "已执行 2 条语句（返回 3 行，影响 2 行）"
        );
    }

    #[test]
    fn summary_without_data_is_explicit() {
        let results = vec![statement(1, 0, None)];

        assert_eq!(summarize(&results), "已执行 1 条语句（无返回数据）");
    }
}
