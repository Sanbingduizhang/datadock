//! 数据库级导入导出业务（结构 / 数据 / 结构+数据）。
//!
//! 导出：按引擎分发到 MySQL / PG 适配器，流式写 SQL 脚本。
//! 导入：直接读盘（不受 SQL 文件 5MB 上限约束），按语句切分后分批执行，
//!       整体包在一个事务里，任一语句失败则回滚，保证导入原子性。

use std::sync::Arc;
use std::time::Instant;

use crate::adapters::mysql::dump as mysql_dump;
use crate::adapters::pg::dump as pg_dump;
use crate::core::{ConnectionManager, PgManager};
use crate::errors::{error_code, AppError};
use crate::models::export::{
    DumpDatabaseRequest, DumpDatabaseResponse, ImportDatabaseRequest, ImportDatabaseResponse,
};
use crate::utils::sql::split_statements;

/// 单批执行的语句数：避免一次性把超大脚本全部读进内存执行
const IMPORT_BATCH_SIZE: usize = 200;

pub struct DumpService {
    manager: Arc<ConnectionManager>,
    pg_manager: Arc<PgManager>,
}

impl DumpService {
    pub fn new(manager: Arc<ConnectionManager>, pg_manager: Arc<PgManager>) -> Self {
        Self {
            manager,
            pg_manager,
        }
    }

    /// 导出数据库（结构 / 数据 / 结构+数据）为 SQL 脚本。
    pub async fn dump(
        &self,
        request: DumpDatabaseRequest,
    ) -> Result<DumpDatabaseResponse, AppError> {
        if request.database_name.trim().is_empty() {
            return Err(AppError::validation("数据库名称不能为空"));
        }

        let tables = request.tables.as_deref();

        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            return pg_dump::dump_schema(
                &pg.pool,
                &request.database_name,
                tables,
                request.mode,
                &request.path,
            )
            .await;
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

        let pool = connection.pool_for(Some(&request.database_name)).await?;

        mysql_dump::dump_database(
            &pool,
            &request.database_name,
            tables,
            request.mode,
            &request.path,
        )
        .await
    }

    /// 从 SQL 脚本导入到目标库：整体一个事务，任一语句失败即回滚。
    pub async fn import(
        &self,
        request: ImportDatabaseRequest,
    ) -> Result<ImportDatabaseResponse, AppError> {
        if request.path.trim().is_empty() {
            return Err(AppError::validation("导入文件路径不能为空"));
        }

        let content = std::fs::read_to_string(&request.path)
            .map_err(|error| AppError::storage(format!("无法读取导入文件：{error}")))?;

        let statements = split_statements(&content);
        if statements.is_empty() {
            return Err(AppError::validation("导入文件中没有可执行的 SQL 语句"));
        }

        let started = Instant::now();

        // PG 与 MySQL 分别走对应执行通道
        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            let count = self.run_pg_import(&pg.pool, &statements).await?;
            return Ok(ImportDatabaseResponse {
                statements: count,
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
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

        // MySQL：优先用目标库连接池；未指定库则复用主池
        let pool = match request.database_name.as_deref() {
            Some(database) if !database.trim().is_empty() => {
                connection.pool_for(Some(database)).await?
            }
            _ => connection.pool.clone(),
        };

        let count = self.run_mysql_import(&pool, &statements).await?;

        Ok(ImportDatabaseResponse {
            statements: count,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// 在事务内分批执行 MySQL 导入语句，失败回滚。
    async fn run_mysql_import(
        &self,
        pool: &sqlx::MySqlPool,
        statements: &[String],
    ) -> Result<u32, AppError> {
        sqlx::raw_sql("START TRANSACTION")
            .execute(pool)
            .await
            .map_err(|error| {
                AppError::new(
                    error_code::SQL_EXECUTION_FAILED,
                    format!("开启导入事务失败：{error}"),
                )
            })?;

        let mut executed: u32 = 0;
        for batch in statements.chunks(IMPORT_BATCH_SIZE) {
            for statement in batch {
                if let Err(error) = sqlx::raw_sql(statement).execute(pool).await {
                    let _ = sqlx::raw_sql("ROLLBACK").execute(pool).await;
                    return Err(AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("导入执行失败：{error}（已回滚）"),
                    ));
                }
                executed += 1;
            }
        }

        if let Err(error) = sqlx::raw_sql("COMMIT").execute(pool).await {
            let _ = sqlx::raw_sql("ROLLBACK").execute(pool).await;
            return Err(AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("提交导入失败：{error}（已回滚）"),
            ));
        }

        Ok(executed)
    }

    /// 在事务内分批执行 PG 导入语句，失败回滚。
    async fn run_pg_import(
        &self,
        pool: &sqlx::PgPool,
        statements: &[String],
    ) -> Result<u32, AppError> {
        sqlx::raw_sql("BEGIN")
            .execute(pool)
            .await
            .map_err(|error| {
                AppError::new(
                    error_code::SQL_EXECUTION_FAILED,
                    format!("开启导入事务失败：{error}"),
                )
            })?;

        let mut executed: u32 = 0;
        for batch in statements.chunks(IMPORT_BATCH_SIZE) {
            for statement in batch {
                if let Err(error) = sqlx::raw_sql(statement).execute(pool).await {
                    let _ = sqlx::raw_sql("ROLLBACK").execute(pool).await;
                    return Err(AppError::new(
                        error_code::SQL_EXECUTION_FAILED,
                        format!("导入执行失败：{error}（已回滚）"),
                    ));
                }
                executed += 1;
            }
        }

        if let Err(error) = sqlx::raw_sql("COMMIT").execute(pool).await {
            let _ = sqlx::raw_sql("ROLLBACK").execute(pool).await;
            return Err(AppError::new(
                error_code::SQL_EXECUTION_FAILED,
                format!("提交导入失败：{error}（已回滚）"),
            ));
        }

        Ok(executed)
    }
}
