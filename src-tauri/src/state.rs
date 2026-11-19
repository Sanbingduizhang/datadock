use std::sync::Arc;

use crate::core::{close_connection, ConnectionManager, PgManager, RedisManager};
use crate::credentials::CredentialStore;
use crate::database::Database;
use crate::services::{
    AppService, ConnectionService, DumpService, ExplorerService, FavoriteService, HistoryService,
    QueryService, RedisService, SchemaService, SettingsService, TableService, WorkspaceService,
};

/// 应用全局状态，通过 `tauri::State` 注入到各 Command。
///
/// 各服务在构造时完成依赖装配，共享同一个 `ConnectionManager` 与 `Database`。
/// 连接池按连接隔离，不使用单一全局锁承载所有数据源操作。
pub struct AppState {
    pub app: Arc<AppService>,
    pub settings: Arc<SettingsService>,
    pub workspace: Arc<WorkspaceService>,
    pub connections: Arc<ConnectionService>,
    pub explorer: Arc<ExplorerService>,
    pub schema: Arc<SchemaService>,
    pub query: Arc<QueryService>,
    pub history: Arc<HistoryService>,
    pub table: Arc<TableService>,
    pub dump: Arc<DumpService>,
    pub favorites: Arc<FavoriteService>,
    pub redis: Arc<RedisService>,
    /// 运行时连接表，退出时统一释放
    pub manager: Arc<ConnectionManager>,
    pub redis_manager: Arc<RedisManager>,
    pub pg_manager: Arc<PgManager>,
    pub credentials: Arc<dyn CredentialStore>,
    pub database: Database,
}

impl AppState {
    pub fn new(
        app: Arc<AppService>,
        database: Database,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        let manager = Arc::new(ConnectionManager::new());
        let redis_manager = Arc::new(RedisManager::new());
        let pg_manager = Arc::new(PgManager::new());
        let redis = Arc::new(RedisService::new(redis_manager.clone()));

        let connections = Arc::new(ConnectionService::new(
            database.clone(),
            credentials.clone(),
            manager.clone(),
            redis_manager.clone(),
            pg_manager.clone(),
        ));
        let explorer = Arc::new(ExplorerService::new(manager.clone(), pg_manager.clone()));
        let schema = Arc::new(SchemaService::new(manager.clone(), pg_manager.clone()));
        let query = Arc::new(QueryService::new(
            manager.clone(),
            pg_manager.clone(),
            database.clone(),
        ));
        let history = Arc::new(HistoryService::new(database.clone()));
        let table = Arc::new(TableService::new(manager.clone(), pg_manager.clone()));
        let dump = Arc::new(DumpService::new(manager.clone(), pg_manager.clone()));
        let favorites = Arc::new(FavoriteService::new(database.clone()));
        let settings = Arc::new(SettingsService::new(database.clone()));
        let workspace = Arc::new(WorkspaceService::new(database.clone()));

        Self {
            app,
            settings,
            workspace,
            connections,
            explorer,
            schema,
            query,
            history,
            table,
            dump,
            favorites,
            redis,
            manager,
            redis_manager,
            pg_manager,
            credentials,
            database,
        }
    }

    /// 释放运行时资源：断开全部数据库连接并关闭本地连接池。
    pub async fn shutdown(&self) {
        let drained = self.manager.drain().await;
        for (connection_id, connection) in drained {
            tracing::info!(target: "datadock::shutdown", connection_id = %connection_id, "关闭运行时连接");
            close_connection(connection).await;
        }

        for (connection_id, _connection) in self.redis_manager.drain().await {
            tracing::info!(target: "datadock::shutdown", connection_id = %connection_id, "关闭 Redis 连接");
        }

        self.database.close().await;
    }
}
