//! 本地 SQLite 数据库：连接配置与迁移执行。

use std::path::Path;
use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::errors::AppError;

/// 编译期嵌入的迁移脚本（src-tauri/migrations）
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// SQLite 连接池持有者。
///
/// 只用于 DataDock 自身数据；远程数据库连接由后续阶段的数据源适配器管理。
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// 打开（或创建）应用数据目录下的数据库文件。
    pub async fn connect(path: &Path) -> Result<Self, AppError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(|error| AppError::storage(format!("打开本地数据库失败：{error}")))?;

        Ok(Self { pool })
    }

    /// 内存数据库，仅用于测试。
    ///
    /// 单连接保证迁移与查询命中同一份内存数据。
    #[cfg(test)]
    pub async fn in_memory() -> Result<Self, AppError> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|error| AppError::storage(format!("创建内存数据库失败：{error}")))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 执行所有未完成的迁移。SQLx 会自行维护迁移记录表。
    pub async fn migrate(&self) -> Result<(), AppError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|error| AppError::storage(format!("本地数据库迁移失败：{error}")))
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrate_creates_all_tables() {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");

        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx%' ORDER BY name",
        )
        .fetch_all(database.pool())
        .await
        .expect("应能读取表列表");

        let names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();
        assert_eq!(
            names,
            vec![
                "app_settings".to_string(),
                "data_sources".to_string(),
                "query_history".to_string(),
                "sql_favorites".to_string(),
                "workspace_state".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("首次迁移应成功");
        database.migrate().await.expect("重复迁移应成功且不报错");
    }

    #[tokio::test]
    async fn connect_creates_database_file() {
        let dir =
            std::env::temp_dir().join(format!("datadock-test-{}", crate::utils::time::new_id()));
        std::fs::create_dir_all(&dir).expect("临时目录应可创建");
        let path = dir.join("datadock.db");

        {
            let database = Database::connect(&path).await.expect("应能创建数据库文件");
            database.migrate().await.expect("迁移应成功");

            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'")
                    .fetch_one(database.pool())
                    .await
                    .expect("应能查询表数量");
            assert!(count.0 >= 5);

            database.close().await;
        }

        assert!(path.exists(), "数据库文件应已落盘");

        // 清理测试文件（含 WAL/SHM）
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(dir.join(format!("datadock.db{suffix}")));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
