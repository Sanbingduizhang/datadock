//! 数据库资源查询业务（数据库 / 表 / 视图）。

use std::sync::Arc;

use crate::adapters::mysql::explorer as mysql_explorer;
use crate::adapters::pg::explorer as pg_explorer;
use crate::core::{ConnectionManager, ManagedConnection, PgManager};
use crate::errors::{error_code, AppError};
use crate::models::database::{DatabaseInfo, TableInfo};

pub struct ExplorerService {
    manager: Arc<ConnectionManager>,
    pg_manager: Arc<PgManager>,
}

impl ExplorerService {
    pub fn new(manager: Arc<ConnectionManager>, pg_manager: Arc<PgManager>) -> Self {
        Self {
            manager,
            pg_manager,
        }
    }

    async fn connection(&self, connection_id: &str) -> Result<Arc<ManagedConnection>, AppError> {
        self.manager.get(connection_id).await.ok_or_else(|| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                "连接尚未建立，请先连接数据库",
            )
        })
    }

    /// 列出资源树的第二层节点。
    ///
    /// MySQL 返回物理数据库；PostgreSQL 返回当前库下的用户 schema。
    /// 分发依据与 `QueryService` 一致：connection_id 命中哪个运行时管理器即为哪种引擎，
    /// 不需要额外读取连接配置表。
    pub async fn list_databases(&self, connection_id: &str) -> Result<Vec<DatabaseInfo>, AppError> {
        if let Some(pg) = self.pg_manager.get(connection_id).await {
            let schemas = pg_explorer::list_schemas(&pg.pool).await?;
            return Ok(schemas
                .into_iter()
                .map(|name| DatabaseInfo { name })
                .collect());
        }

        let connection = self.connection(connection_id).await?;
        mysql_explorer::list_databases(&connection.pool).await
    }

    /// 列出某个库（MySQL）或 schema（PostgreSQL）下的表与视图。
    pub async fn list_tables(
        &self,
        connection_id: &str,
        database: &str,
    ) -> Result<Vec<TableInfo>, AppError> {
        let pg = self.pg_manager.get(connection_id).await;

        if database.trim().is_empty() {
            let label = if pg.is_some() { "schema" } else { "数据库" };
            return Err(AppError::validation(format!("{label}名称不能为空")));
        }

        if let Some(pg) = pg {
            return pg_explorer::list_tables(&pg.pool, database).await;
        }

        let connection = self.connection(connection_id).await?;
        mysql_explorer::list_tables(&connection.pool, database).await
    }
}
