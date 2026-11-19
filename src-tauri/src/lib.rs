//! DataDock 后端入口。
//!
//! 分层约定（详见 `docs/07-项目目录结构与代码规范.md`）：
//!
//! ```text
//! Command（IPC 入口） → Service（业务编排） → Repository / Adapter（数据访问）
//! ```

pub mod adapters;
pub mod commands;
pub mod core;
pub mod credentials;
pub mod database;
pub mod errors;
pub mod models;
pub mod repositories;
pub mod services;
pub mod state;
pub mod utils;

mod logging;

use std::sync::Arc;

use tauri::{Manager, RunEvent};

use crate::credentials::{CredentialStore, KeyringCredentialStore, SERVICE_NAME};
use crate::database::Database;
use crate::errors::AppError;
use crate::models::settings::DEFAULT_SETTINGS;
use crate::repositories::settings_repository;
use crate::services::AppService;
use crate::state::AppState;

const DB_FILE_NAME: &str = "datadock.db";

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // 1. 日志：需要应用路径解析能力，因此放在 setup 中初始化
            let log_dir = logging::init(app.handle())?;

            // 2. 应用数据目录与本地数据库
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let database = tauri::async_runtime::block_on(init_database(&data_dir))?;

            // 3. 服务与全局状态
            let app_service = Arc::new(AppService::new(data_dir.clone(), log_dir.clone()));
            let credentials: Arc<dyn CredentialStore> =
                Arc::new(KeyringCredentialStore::new(SERVICE_NAME));

            app.manage(AppState::new(app_service, database, credentials));

            tracing::info!(
                target: "datadock::startup",
                version = env!("CARGO_PKG_VERSION"),
                os = std::env::consts::OS,
                arch = std::env::consts::ARCH,
                data_dir = %data_dir.display(),
                log_dir = %log_dir.display(),
                "DataDock 启动完成"
            );

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::ping,
            commands::app::get_app_info,
            commands::settings::get_all_settings,
            commands::settings::update_settings,
            commands::workspace::get_workspace,
            commands::workspace::save_workspace,
            commands::workspace::list_workspaces,
            commands::workspace::delete_workspace,
            commands::connection::list_connections,
            commands::connection::create_connection,
            commands::connection::update_connection,
            commands::connection::delete_connection,
            commands::connection::test_connection,
            commands::connection::connect_database,
            commands::connection::disconnect_database,
            commands::connection::get_connection_status,
            commands::connection::export_connections,
            commands::connection::import_connections,
            commands::explorer::list_databases,
            commands::explorer::list_tables,
            commands::schema::get_table_schema,
            commands::schema::get_database_ddl,
            commands::query::execute_sql,
            commands::query::execute_batch_sql,
            commands::query::cancel_task,
            commands::query::export_query_result,
            commands::query::begin_transaction,
            commands::query::commit_transaction,
            commands::query::rollback_transaction,
            commands::history::list_query_history,
            commands::history::delete_query_history,
            commands::history::clear_query_history,
            commands::table::get_table_data,
            commands::table::export_table_data,
            commands::dump::dump_database,
            commands::dump::import_database,
            commands::favorite::list_sql_favorites,
            commands::favorite::create_sql_favorite,
            commands::favorite::update_sql_favorite,
            commands::favorite::delete_sql_favorite,
            commands::redis::redis_execute,
            commands::redis::redis_list_databases,
            commands::redis::redis_scan_keys,
            commands::redis::redis_get_key_value,
            commands::redis::redis_delete_key,
            commands::redis::redis_rename_key,
            commands::redis::redis_set_ttl,
            commands::redis::redis_set_key_value,
            commands::file::read_sql_file,
            commands::file::write_sql_file,
        ])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|handle, event| {
            if let RunEvent::Exit = event {
                // 正常退出：断开全部远程连接并关闭本地连接池
                let state = handle.state::<AppState>();
                tauri::async_runtime::block_on(state.shutdown());
                tracing::info!(target: "datadock::shutdown", "DataDock 退出");
            }
        }),
        Err(error) => {
            // 此时日志系统可能尚未初始化，只能输出到标准错误
            eprintln!("DataDock 启动失败：{error}");
            std::process::exit(1);
        }
    }
}

/// 打开本地数据库、执行迁移并写入默认设置。
///
/// 迁移失败时整体启动失败，避免在结构不完整的状态下继续运行。
async fn init_database(data_dir: &std::path::Path) -> Result<Database, AppError> {
    let db_path = data_dir.join(DB_FILE_NAME);
    let database = Database::connect(&db_path).await?;
    database.migrate().await?;
    settings_repository::ensure_defaults(database.pool(), DEFAULT_SETTINGS).await?;

    tracing::info!(
        target: "datadock::storage",
        path = %db_path.display(),
        "本地数据库已就绪"
    );

    Ok(database)
}
