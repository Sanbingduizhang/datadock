//! 表结构查询业务。

use std::sync::Arc;

use crate::adapters::mysql::schema as mysql_schema;
use crate::adapters::pg::schema as pg_schema;
use crate::core::{ConnectionManager, ManagedConnection, PgManager};
use crate::errors::{error_code, AppError};
use crate::models::database::{DatabaseDdl, TableSchema};

pub struct SchemaService {
    manager: Arc<ConnectionManager>,
    pg_manager: Arc<PgManager>,
}

impl SchemaService {
    pub fn new(manager: Arc<ConnectionManager>, pg_manager: Arc<PgManager>) -> Self {
        Self {
            manager,
            pg_manager,
        }
    }

    async fn mysql_connection(
        &self,
        connection_id: &str,
    ) -> Result<Arc<ManagedConnection>, AppError> {
        self.manager.get(connection_id).await.ok_or_else(|| {
            AppError::new(
                error_code::CONNECTION_NOT_READY,
                "连接尚未建立，请先连接数据库",
            )
        })
    }

    /// 读取表结构。
    ///
    /// MySQL 的 `database` 为物理库名；PostgreSQL 的 `database` 为 schema 名。
    /// 分发依据与 `QueryService`/`ExplorerService` 一致：connection_id 命中哪个运行时
    /// 管理器即为哪种引擎，不需要额外读取连接配置表。
    pub async fn table_schema(
        &self,
        connection_id: &str,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, AppError> {
        if database.trim().is_empty() {
            return Err(AppError::validation("数据库名称不能为空"));
        }
        if table.trim().is_empty() {
            return Err(AppError::validation("表名称不能为空"));
        }

        if let Some(pg) = self.pg_manager.get(connection_id).await {
            return pg_schema::table_schema(&pg.pool, database, table).await;
        }

        let connection = self.mysql_connection(connection_id).await?;
        mysql_schema::table_schema(&connection.pool, database, table).await
    }

    /// 读取建库语句，便于查看或导出库的定义。
    ///
    /// PostgreSQL 返回说明性文案：物理库无法直接导出 `CREATE DATABASE`，
    /// 提示改用 `pg_dump --schema-only`；资源树也已对该入口隐藏。
    pub async fn database_ddl(
        &self,
        connection_id: &str,
        database: &str,
    ) -> Result<DatabaseDdl, AppError> {
        if database.trim().is_empty() {
            return Err(AppError::validation("数据库名称不能为空"));
        }

        if let Some(pg) = self.pg_manager.get(connection_id).await {
            let ddl = pg_schema::database_ddl(&pg.pool).await?;
            return Ok(DatabaseDdl {
                database_name: database.to_string(),
                ddl,
            });
        }

        let connection = self.mysql_connection(connection_id).await?;
        let ddl = mysql_schema::database_ddl(&connection.pool, database).await?;

        Ok(DatabaseDdl {
            database_name: database.to_string(),
            ddl,
        })
    }
}
