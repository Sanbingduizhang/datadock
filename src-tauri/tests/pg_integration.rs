//! PostgreSQL 资源树集成测试（PG 适配器第 1 项）。
//!
//! 需要本地 PostgreSQL（默认 127.0.0.1:5432，用户 fxl 免密）。
//! 参数可用 DATADOCK_PG_* 覆盖。运行：
//!
//! ```bash
//! cargo test --test pg_integration -- --ignored --nocapture
//! ```
//!
//! 测试只操作自建的临时 schema，结束时 DROP CASCADE 清理。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use datadock_lib::core::{ConnectionManager, PgManager, RedisManager};
use datadock_lib::credentials::CredentialStore;
use datadock_lib::database::Database;
use datadock_lib::errors::AppError;
use datadock_lib::models::connection::CreateConnectionRequest;
use datadock_lib::models::database::TableKind;
use datadock_lib::models::export::{ExportFormat, ExportTableRequest};
use datadock_lib::models::query::{BeginTransactionRequest, ExecuteSqlRequest, TransactionRequest};
use datadock_lib::models::table::TableDataRequest;
use datadock_lib::services::{
    ConnectionService, ExplorerService, QueryService, SchemaService, TableService,
};

#[derive(Default)]
struct InMemoryCredentialStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl CredentialStore for InMemoryCredentialStore {
    fn set_secret(&self, key: &str, secret: &str) -> Result<(), AppError> {
        self.secrets
            .lock()
            .map_err(|_| AppError::credential("凭据锁已损坏"))?
            .insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| AppError::credential("凭据锁已损坏"))?
            .get(key)
            .cloned())
    }

    fn delete_secret(&self, key: &str) -> Result<(), AppError> {
        self.secrets
            .lock()
            .map_err(|_| AppError::credential("凭据锁已损坏"))?
            .remove(key);
        Ok(())
    }
}

struct PgEnv {
    host: String,
    port: u16,
    user: String,
    password: String,
    database: String,
}

fn pg_env() -> PgEnv {
    fn value(name: &str, default: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| default.to_string())
    }

    PgEnv {
        host: value("DATADOCK_PG_HOST", "127.0.0.1"),
        port: value("DATADOCK_PG_PORT", "5432").parse().unwrap_or(5432),
        user: value("DATADOCK_PG_USER", "fxl"),
        password: value("DATADOCK_PG_PASSWORD", ""),
        database: value("DATADOCK_PG_DB", "postgres"),
    }
}

fn pg_options(env: &PgEnv) -> PgConnectOptions {
    let mut options = PgConnectOptions::new()
        .host(&env.host)
        .port(env.port)
        .username(&env.user)
        .database(&env.database);
    if !env.password.is_empty() {
        options = options.password(&env.password);
    }
    options
}

async fn admin_pool(env: &PgEnv) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect_with(pg_options(env))
        .await
        .expect("应能连接本地 PostgreSQL")
}

async fn services() -> (
    ConnectionService,
    ExplorerService,
    SchemaService,
    TableService,
    QueryService,
    PathBuf,
) {
    let dir = std::env::temp_dir().join(format!(
        "datadock-pg-{}",
        datadock_lib::utils::time::new_id()
    ));
    std::fs::create_dir_all(&dir).expect("临时目录应可创建");
    let database = Database::connect(&dir.join("test.db"))
        .await
        .expect("本地库应可创建");
    database.migrate().await.expect("迁移应成功");

    let credentials: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::default());
    let manager = Arc::new(ConnectionManager::new());
    let pg_manager = Arc::new(PgManager::new());

    let connections = ConnectionService::new(
        database.clone(),
        credentials,
        manager.clone(),
        Arc::new(RedisManager::new()),
        pg_manager.clone(),
    );
    let explorer = ExplorerService::new(manager.clone(), pg_manager.clone());
    let schema = SchemaService::new(manager.clone(), pg_manager.clone());
    let table = TableService::new(manager.clone(), pg_manager.clone());
    let query = QueryService::new(manager, pg_manager, database);

    (connections, explorer, schema, table, query, dir)
}

#[tokio::test]
#[ignore = "需要本地 PostgreSQL"]
async fn pg_explorer_dispatches_schemas_and_tables() {
    let env = pg_env();
    let admin = admin_pool(&env).await;

    let schema = format!("datadock_it_{}", std::process::id());
    sqlx::raw_sql(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .execute(&admin)
        .await
        .expect("清理旧测试 schema 应成功");
    sqlx::raw_sql(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&admin)
        .await
        .expect("创建测试 schema 应成功");
    sqlx::raw_sql(&format!(
        "CREATE TABLE \"{schema}\".accounts (id serial PRIMARY KEY, name text NOT NULL, balance numeric(12,2))"
    ))
    .execute(&admin)
    .await
    .expect("创建测试表应成功");
    sqlx::raw_sql(&format!(
        "CREATE UNIQUE INDEX \"{schema}_accounts_name_key\" ON \"{schema}\".accounts (name)"
    ))
    .execute(&admin)
    .await
    .expect("创建测试唯一索引应成功");
    sqlx::raw_sql(&format!(
        "CREATE VIEW \"{schema}\".active_accounts AS SELECT id, name FROM \"{schema}\".accounts WHERE name IS NOT NULL"
    ))
    .execute(&admin)
    .await
    .expect("创建测试视图应成功");

    // 写入样例数据，供表数据浏览 / 导出验证
    for (name, balance) in [("alice", "10.50"), ("bob", "20.25"), ("carol", "30.75")] {
        sqlx::query(&format!(
            "INSERT INTO \"{schema}\".accounts (name, balance) VALUES ($1, $2::numeric)"
        ))
        .bind(name)
        .bind(balance)
        .execute(&admin)
        .await
        .expect("写入样例数据应成功");
    }

    let (connections, explorer, schema_service, table_service, query, dir) = services().await;

    // 1. 创建 PG 配置并连接
    let created = connections
        .create(CreateConnectionRequest {
            name: "IT-PG".to_string(),
            source_type: "postgres".to_string(),
            host: env.host.clone(),
            port: env.port,
            username: env.user.clone(),
            password: env.password.clone(),
            default_database: Some(env.database.clone()),
            charset: None,
            connect_timeout_ms: Some(5_000),
            options: None,
        })
        .await
        .expect("创建 PG 连接配置应成功");

    let connected = connections
        .connect(&created.connection_id)
        .await
        .expect("连接 PostgreSQL 应成功");
    assert_eq!(connected.status, "connected");
    println!(
        "pg connected: version={:?} database={:?}",
        connected.server_version, connected.current_database
    );

    // 2. 连接列表必须反映 PG 运行时状态（否则资源树展开会被判为未连接）
    let listed = connections.list().await.expect("连接列表应可读取");
    let info = listed
        .iter()
        .find(|item| item.id == created.connection_id)
        .expect("应能找到刚创建的 PG 连接");
    assert_eq!(info.source_type, "postgres");
    assert!(info.connected, "PG 连接应被标记为已连接");
    assert!(info.server_version.is_some(), "PG 版本应回填");

    // 3. 第二层节点 = schema 列表（排除系统 schema）
    let databases = explorer
        .list_databases(&created.connection_id)
        .await
        .expect("schema 列表应可读取");
    println!(
        "pg schemas: {:?}",
        databases.iter().map(|item| &item.name).collect::<Vec<_>>()
    );
    assert!(
        databases.iter().any(|item| item.name == schema),
        "应包含测试 schema：{databases:?}"
    );
    assert!(
        databases.iter().all(|item| !item.name.starts_with("pg_")),
        "不应包含系统 schema"
    );
    assert!(
        databases
            .iter()
            .all(|item| item.name != "information_schema"),
        "不应包含 information_schema"
    );

    // 4. 表与视图分类
    let tables = explorer
        .list_tables(&created.connection_id, &schema)
        .await
        .expect("表列表应可读取");
    println!(
        "pg objects: {:?}",
        tables
            .iter()
            .map(|item| (&item.name, item.table_kind))
            .collect::<Vec<_>>()
    );
    assert_eq!(tables.len(), 2, "测试 schema 应有一张表与一个视图");
    let account = tables
        .iter()
        .find(|item| item.name == "accounts")
        .expect("应有 accounts 表");
    assert_eq!(account.table_kind, TableKind::Table);
    let view = tables
        .iter()
        .find(|item| item.name == "active_accounts")
        .expect("应有 active_accounts 视图");
    assert_eq!(view.table_kind, TableKind::View);

    // 5. 表结构（SchemaService 分发到 pg_schema）
    let structure = schema_service
        .table_schema(&created.connection_id, &schema, "accounts")
        .await
        .expect("表结构应可读取");
    println!(
        "pg schema columns: {:?}",
        structure
            .columns
            .iter()
            .map(|column| (
                &column.name,
                &column.column_type,
                &column.column_key,
                column.nullable
            ))
            .collect::<Vec<_>>()
    );
    println!("pg schema indexes: {:?}", structure.indexes);

    let id_column = structure
        .columns
        .iter()
        .find(|column| column.name == "id")
        .expect("应有 id 字段");
    assert_eq!(id_column.column_key, "PRI", "id 应为主键");
    assert!(!id_column.nullable, "id 不可空");
    let name_column = structure
        .columns
        .iter()
        .find(|column| column.name == "name")
        .expect("应有 name 字段");
    assert!(!name_column.nullable, "name 不可空");
    assert!(
        structure
            .columns
            .iter()
            .any(|column| column.name == "balance"),
        "应有 balance 字段"
    );
    assert!(
        structure.indexes.iter().any(|index| index.unique),
        "应读到 name 上的唯一索引"
    );
    assert!(
        structure.ddl.to_uppercase().contains("CREATE TABLE"),
        "PG 表结构应返回建表 DDL：{}",
        structure.ddl
    );

    // 视图结构应返回视图定义
    let view_structure = schema_service
        .table_schema(&created.connection_id, &schema, "active_accounts")
        .await
        .expect("视图结构应可读取");
    assert!(
        view_structure
            .ddl
            .to_uppercase()
            .contains("CREATE OR REPLACE VIEW"),
        "PG 视图结构应返回视图定义：{}",
        view_structure.ddl
    );

    // 不存在的表应报错
    let missing = schema_service
        .table_schema(&created.connection_id, &schema, "no_such_table")
        .await
        .expect_err("不存在的表应报错");
    assert!(
        missing.message.contains("不存在"),
        "错误文案应提示表不存在：{}",
        missing.message
    );

    // 5.1 表数据浏览（TableService 分发到 pg_table）
    let page1 = table_service
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            page: Some(1),
            page_size: Some(2),
            order_by: Some("id".to_string()),
            order_desc: Some(false),
            filter: None,
        })
        .await
        .expect("表数据第一页应可读取");
    println!(
        "pg table page1 columns: {:?} rows: {:?} total: {}",
        page1
            .columns
            .iter()
            .map(|column| &column.name)
            .collect::<Vec<_>>(),
        page1.rows,
        page1.total_rows
    );
    assert_eq!(page1.total_rows, 3, "样例数据共 3 行");
    assert_eq!(page1.page_size, 2, "每页应为 2 行");
    assert_eq!(page1.rows.len(), 2, "第一页应返回 2 行");
    assert_eq!(page1.primary_key, vec!["id".to_string()], "主键应为 id");

    // 排序列必须真实存在，否则拒绝
    let bad_order = table_service
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            page: None,
            page_size: None,
            order_by: Some("no_such_column".to_string()),
            order_desc: None,
            filter: None,
        })
        .await
        .expect_err("不存在的排序列应被拒绝");
    assert!(
        bad_order.message.contains("不存在"),
        "错误文案应提示排序列不存在：{}",
        bad_order.message
    );

    // 筛选条件
    let filtered = table_service
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            page: None,
            page_size: None,
            order_by: None,
            order_desc: None,
            filter: Some("name = 'alice'".to_string()),
        })
        .await
        .expect("筛选查询应可执行");
    assert_eq!(filtered.total_rows, 1, "name = alice 应只有一行");
    assert_eq!(filtered.rows.len(), 1, "筛选后应返回 1 行");

    // 筛选条件不允许分号
    let bad_filter = table_service
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            page: None,
            page_size: None,
            order_by: None,
            order_desc: None,
            filter: Some("1=1; DROP TABLE accounts".to_string()),
        })
        .await
        .expect_err("带分号的筛选条件应被拒绝");
    assert!(
        bad_filter.message.contains("分号"),
        "错误文案应拒绝分号：{}",
        bad_filter.message
    );

    // 5.2 表数据导出（CSV / JSON）
    let csv_path = dir.join("accounts.csv").to_string_lossy().to_string();
    let csv_result = table_service
        .export(ExportTableRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            format: ExportFormat::Csv,
            path: csv_path.clone(),
            filter: None,
            order_by: None,
            order_desc: None,
        })
        .await
        .expect("CSV 导出应成功");
    assert_eq!(csv_result.rows_written, 3, "CSV 应导出 3 行");
    let csv_text = std::fs::read_to_string(&csv_path).expect("CSV 文件应可读");
    assert!(
        csv_text.contains("name") && csv_text.contains("balance"),
        "CSV 应包含表头"
    );
    assert!(csv_text.contains("10.50"), "CSV 应保留 numeric 精度");
    println!(
        "pg export csv: {} rows={}",
        csv_path, csv_result.rows_written
    );

    let json_path = dir.join("accounts.json").to_string_lossy().to_string();
    let json_result = table_service
        .export(ExportTableRequest {
            connection_id: created.connection_id.clone(),
            database_name: schema.clone(),
            table_name: "accounts".to_string(),
            format: ExportFormat::Json,
            path: json_path.clone(),
            filter: None,
            order_by: None,
            order_desc: None,
        })
        .await
        .expect("JSON 导出应成功");
    assert_eq!(json_result.rows_written, 3, "JSON 应导出 3 行");
    let json_text = std::fs::read_to_string(&json_path).expect("JSON 文件应可读");
    let parsed: serde_json::Value = serde_json::from_str(&json_text).expect("JSON 应可解析");
    assert_eq!(
        parsed.as_array().map(|array| array.len()),
        Some(3),
        "JSON 应为 3 个对象的数组"
    );

    // 6. 空 schema（public）与非法参数
    let public_tables = explorer
        .list_tables(&created.connection_id, "public")
        .await
        .expect("public schema 应可读取");
    assert!(public_tables.is_empty(), "本机 public 下应无表");

    let rejected = explorer
        .list_tables(&created.connection_id, "  ")
        .await
        .expect_err("空 schema 名应被拒绝");
    assert!(
        rejected.message.contains("schema"),
        "错误文案应按引擎提示 schema：{}",
        rejected.message
    );

    // 6.9 手动事务：未提交对其他会话不可见，回滚后消失，提交后保留
    let tx = query
        .begin_transaction(BeginTransactionRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
        })
        .await
        .expect("PG 开启事务应成功");
    println!("pg transaction_id: {}", tx.transaction_id);

    // 事务内写入
    let insert_in_tx = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: format!(
                "INSERT INTO \"{schema}\".accounts (name, balance) VALUES ('tx_rollback', 0::numeric)"
            ),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: Some(tx.transaction_id.clone()),
        })
        .await
        .expect("PG 事务内写入应成功");
    assert!(insert_in_tx.error.is_none(), "事务内 INSERT 不应报错");

    // 未提交数据对主池（其他会话）不可见
    let outside = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: format!("SELECT COUNT(*) FROM \"{schema}\".accounts WHERE name = 'tx_rollback'"),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("外部查询应成功");
    assert_eq!(
        outside.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("0"),
        "PG 未提交数据对其他会话不可见"
    );

    // 回滚
    let rolled = query
        .rollback_transaction(TransactionRequest {
            connection_id: created.connection_id.clone(),
            transaction_id: tx.transaction_id.clone(),
        })
        .await
        .expect("PG 回滚应成功");
    assert!(rolled.success);
    assert_eq!(rolled.message, "事务已回滚");

    let after_rollback = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: format!("SELECT COUNT(*) FROM \"{schema}\".accounts WHERE name = 'tx_rollback'"),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("回滚后查询应成功");
    assert_eq!(
        after_rollback.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("0"),
        "PG 回滚后数据应不存在"
    );

    // 提交事务
    let tx2 = query
        .begin_transaction(BeginTransactionRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
        })
        .await
        .expect("PG 开启第二个事务应成功");
    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: format!(
                "INSERT INTO \"{schema}\".accounts (name, balance) VALUES ('tx_commit', 1::numeric)"
            ),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: Some(tx2.transaction_id.clone()),
        })
        .await
        .expect("PG 事务内写入应成功");
    let committed = query
        .commit_transaction(TransactionRequest {
            connection_id: created.connection_id.clone(),
            transaction_id: tx2.transaction_id.clone(),
        })
        .await
        .expect("PG 提交应成功");
    assert!(committed.success);
    assert_eq!(committed.message, "事务已提交");

    let after_commit = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: format!("SELECT COUNT(*) FROM \"{schema}\".accounts WHERE name = 'tx_commit'"),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("提交后查询应成功");
    assert_eq!(
        after_commit.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("1"),
        "PG 提交后数据应存在"
    );

    // 失效事务（已被提交/回滚）再次使用应报错
    let stale_tx = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(env.database.clone()),
            sql: "SELECT 1".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: Some(tx2.transaction_id.clone()),
        })
        .await
        .expect_err("已提交的事务会话不存在应报错");
    assert!(
        stale_tx.message.contains("事务会话不存在") || stale_tx.message.contains("已提交或回滚"),
        "错误文案应提示事务失效：{}",
        stale_tx.message
    );

    // 7. 断开后状态回落，删除配置
    connections
        .disconnect(&created.connection_id)
        .await
        .expect("断开应成功");
    let after = connections.list().await.expect("连接列表应可读取");
    assert!(
        !after
            .iter()
            .find(|item| item.id == created.connection_id)
            .expect("配置应仍存在")
            .connected,
        "断开后应标记为未连接"
    );
    connections
        .delete(&created.connection_id)
        .await
        .expect("删除配置应成功");

    sqlx::raw_sql(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .execute(&admin)
        .await
        .expect("清理测试 schema 应成功");
    admin.close().await;
    let _ = std::fs::remove_dir_all(&dir);
}
