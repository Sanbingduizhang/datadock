//! MySQL 端到端集成测试：配置 → 建立连接 → 数据库/表 → 表结构 → 断开 → 删除。
//!
//! 默认 `#[ignore]`，需要真实 MySQL 与可用 Keychain：
//!
//! ```bash
//! DATADOCK_MYSQL_HOST=127.0.0.1 DATADOCK_MYSQL_PORT=3306 \
//! DATADOCK_MYSQL_USER=root DATADOCK_MYSQL_PASSWORD=*** \
//! DATADOCK_MYSQL_DB=tbdock \
//! cargo test --test mysql_integration -- --ignored --nocapture
//! ```
//!
//! 测试会真实写入系统凭据与本地 SQLite（临时文件），结束时清理。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use datadock_lib::core::{ConnectionManager, PgManager, RedisManager};
use datadock_lib::credentials::CredentialStore;
use datadock_lib::database::Database;
use datadock_lib::errors::AppError;
use datadock_lib::models::connection::{
    CreateConnectionRequest, TestConnectionRequest, UpdateConnectionRequest,
};
use datadock_lib::models::export::{ExportFormat, ExportQueryRequest, ExportTableRequest};
use datadock_lib::models::query::{BeginTransactionRequest, ExecuteSqlRequest, TransactionRequest};
use datadock_lib::models::table::TableDataRequest;
use datadock_lib::repositories::history_repository;
use datadock_lib::repositories::settings_repository;
use datadock_lib::services::{
    ConnectionService, ExplorerService, QueryService, SchemaService, TableService,
};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("缺少环境变量 {name}"))
}

/// 测试用的内存凭据存储。
///
/// 系统 Keychain 在 CI / 沙箱环境中不可用（EPERM），因此集成测试注入该实现，
/// 以验证「配置 → 连接 → 元数据 → 断开 → 删除」全链路；
/// 真实 Keychain 由应用运行时验证（`cargo test credentials::` 中的往返测试）。
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

async fn service() -> (
    ConnectionService,
    ExplorerService,
    SchemaService,
    QueryService,
    TableService,
    std::path::PathBuf,
    Database,
) {
    let dir = std::env::temp_dir().join(format!(
        "datadock-it-{}",
        datadock_lib::utils::time::new_id()
    ));
    std::fs::create_dir_all(&dir).expect("临时目录应可创建");
    let path = dir.join("test.db");

    let database = Database::connect(&path).await.expect("本地库应可创建");
    database.migrate().await.expect("迁移应成功");
    settings_repository::ensure_defaults(
        database.pool(),
        datadock_lib::models::settings::DEFAULT_SETTINGS,
    )
    .await
    .expect("默认设置应写入");

    let credentials: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::default());
    let manager = Arc::new(ConnectionManager::new());
    let pg_manager = Arc::new(PgManager::new());

    let connection_service = ConnectionService::new(
        database.clone(),
        credentials,
        manager.clone(),
        Arc::new(RedisManager::new()),
        pg_manager.clone(),
    );

    (
        connection_service,
        ExplorerService::new(manager.clone(), pg_manager.clone()),
        SchemaService::new(manager.clone(), pg_manager.clone()),
        QueryService::new(manager.clone(), pg_manager.clone(), database.clone()),
        TableService::new(manager, pg_manager),
        dir,
        database,
    )
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境与 Keychain"]
async fn full_connection_lifecycle() {
    let (connections, explorer, schema, query, table, dir, database) = service().await;
    let query = Arc::new(query);

    let host = env("DATADOCK_MYSQL_HOST");
    let port: u16 = env("DATADOCK_MYSQL_PORT").parse().expect("端口应为数字");
    let username = env("DATADOCK_MYSQL_USER");
    let password = env("DATADOCK_MYSQL_PASSWORD");
    let database_name = env("DATADOCK_MYSQL_DB");

    // 1. 测试连接（不落库）
    let test_result = connections
        .test(TestConnectionRequest {
            source_type: "mysql".to_string(),
            host: host.clone(),
            port,
            username: username.clone(),
            password: password.clone(),
            default_database: Some(database_name.clone()),
            charset: None,
            connect_timeout_ms: Some(5_000),
            options: None,
        })
        .await
        .expect("测试连接调用本身不应报错");
    println!(
        "test_connection success={} message={} version={:?} elapsed={}ms",
        test_result.success,
        test_result.message,
        test_result.server_version,
        test_result.elapsed_ms
    );
    assert!(
        test_result.success,
        "测试连接应成功：{}",
        test_result.message
    );

    // 2. 创建配置（密码进 Keychain）
    let created = connections
        .create(CreateConnectionRequest {
            name: "IT-本地 MySQL".to_string(),
            source_type: "mysql".to_string(),
            host,
            port,
            username,
            password,
            default_database: Some(database_name.clone()),
            charset: Some("utf8mb4".to_string()),
            connect_timeout_ms: Some(5_000),
            options: None,
        })
        .await
        .expect("创建连接应成功");

    let list = connections.list().await.expect("列表应可读取");
    let info = list
        .iter()
        .find(|item| item.id == created.connection_id)
        .expect("应能查到新连接");
    assert!(!info.connected);
    assert_eq!(info.source_type, "mysql");

    // 3. 建立运行时连接
    let connected = connections
        .connect(&created.connection_id)
        .await
        .expect("连接应成功");
    println!("connected: {connected:?}");
    assert_eq!(connected.status, "connected");

    // 4. 数据库与表
    let databases = explorer
        .list_databases(&created.connection_id)
        .await
        .expect("数据库列表应可读取");
    assert!(databases.iter().any(|db| db.name == database_name));
    println!("databases_count={}", databases.len());

    let tables = explorer
        .list_tables(&created.connection_id, &database_name)
        .await
        .expect("表列表应可读取");
    let names: Vec<&str> = tables.iter().map(|table| table.name.as_str()).collect();
    println!("tables={names:?}");
    assert!(names.contains(&"users"));
    assert!(names.contains(&"v_active_users"));
    assert_eq!(
        tables
            .iter()
            .find(|table| table.name == "v_active_users")
            .map(|table| table.table_kind),
        Some(datadock_lib::models::database::TableKind::View)
    );

    // 5. 表结构
    let users_schema = schema
        .table_schema(&created.connection_id, &database_name, "users")
        .await
        .expect("表结构应可读取");
    println!(
        "users columns={} indexes={} ddl_len={}",
        users_schema.columns.len(),
        users_schema.indexes.len(),
        users_schema.ddl.len()
    );
    assert!(users_schema
        .columns
        .iter()
        .any(|column| column.name == "id" && column.is_primary_key()));
    let uk = users_schema
        .indexes
        .iter()
        .find(|index| index.name == "uk_username")
        .expect("应有唯一索引");
    assert!(uk.unique);
    assert!(users_schema.ddl.contains("CREATE TABLE"));

    // 6. 执行 SQL：类型保真、行数上限、写操作、多语句、取消与历史
    let select_result = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT id, username, balance, profile, created_at FROM users ORDER BY id"
                .to_string(),
            max_rows: Some(100),
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("查询应成功");
    let result_set = select_result
        .results
        .first()
        .and_then(|item| item.result_set.as_ref())
        .expect("应有结果集");
    println!(
        "query columns={:?} rows={} elapsed={}ms",
        result_set
            .columns
            .iter()
            .map(|column| format!("{}:{}", column.name, column.type_name))
            .collect::<Vec<_>>(),
        result_set.row_count,
        select_result.elapsed_ms
    );
    assert_eq!(result_set.row_count, 3);
    assert_eq!(result_set.columns.len(), 5);
    // DECIMAL 必须保留 4 位小数，不能退化为浮点
    assert_eq!(
        result_set.rows[0][2].value.as_deref(),
        Some("1234.5600"),
        "DECIMAL 精度丢失"
    );
    assert_eq!(result_set.rows[0][1].value.as_deref(), Some("alice"));
    // NULL 必须与空字符串区分
    assert_eq!(result_set.rows[1][3].value_type, "null");

    // 空结果集也应带列头
    let empty = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT id, username FROM users WHERE 1 = 0".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("空结果查询应成功");
    let empty_set = empty.results[0].result_set.as_ref().expect("应有结果集");
    assert_eq!(empty_set.row_count, 0);
    assert_eq!(empty_set.columns.len(), 2);
    assert_eq!(empty_set.columns[0].name, "id");

    let limited = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT * FROM users".to_string(),
            max_rows: Some(1),
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("查询应成功");
    assert_eq!(
        limited.results[0]
            .result_set
            .as_ref()
            .map(|set| set.row_count),
        Some(1)
    );
    assert!(limited.truncated, "超过上限时应标记截断");

    let insert = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "INSERT INTO event_log (level, message) VALUES ('info', 'integration-test')"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("写操作应成功");
    assert_eq!(insert.results[0].affected_rows, Some(1));

    // 多语句：按顺序执行，各自返回结果集
    let multi = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT 1 AS a; SELECT 2 AS b;".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("多语句应成功");
    assert_eq!(multi.results.len(), 2);
    assert!(multi.error.is_none());
    assert_eq!(
        multi.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("1")
    );
    assert_eq!(
        multi.results[1]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("2")
    );
    println!("multi summary = {}", multi.message);

    // 中途失败：保留已完成的结果并给出行号
    let partial = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT 1; SELECT * FROM table_that_does_not_exist".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("语句失败应通过 error 字段返回");
    assert_eq!(partial.results.len(), 1);
    let failure = partial.error.as_ref().expect("应返回失败语句信息");
    assert_eq!(failure.index, 2);
    assert!(partial.message.contains("第 2 条语句"));

    // 取消：SLEEP 被 KILL QUERY 中断
    let cancel_task_id = format!("it-cancel-{}", std::process::id());
    let sleep_query = query.clone();
    let sleep_connection = created.connection_id.clone();
    let sleep_database = database_name.clone();
    let sleep_task = cancel_task_id.clone();
    let handle = tokio::spawn(async move {
        sleep_query
            .execute(ExecuteSqlRequest {
                connection_id: sleep_connection,
                database_name: Some(sleep_database),
                sql: "SELECT SLEEP(10)".to_string(),
                max_rows: None,
                task_id: Some(sleep_task),
                source_type: None,
                transaction_id: None,
            })
            .await
    });

    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut cancelled = false;
    for _ in 0..20 {
        if query
            .cancel(&cancel_task_id)
            .await
            .map(|reply| reply.cancelled)
            .unwrap_or(false)
        {
            cancelled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(cancelled, "取消请求应被服务端接受");

    let sleep_outcome = tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("取消后查询应在 5 秒内返回")
        .expect("查询任务不应 panic")
        .expect("取消也应返回响应");
    println!("cancel message = {}", sleep_outcome.message);
    assert!(sleep_outcome.cancelled, "响应应标记为已取消");

    // 历史：每次执行都应有记录，状态与统计正确
    let history_count = history_repository::count(database.pool())
        .await
        .expect("历史统计应成功");
    assert!(history_count >= 5, "查询历史应有记录，实际 {history_count}");
    let history = history_repository::list(database.pool(), 5, 0, None, None)
        .await
        .expect("历史列表应可读取");
    assert!(history.iter().any(|item| item.status == "cancelled"));

    // 6.3.1 表结构设计器生成的 ALTER 语句形态（临时表，测试后删除）
    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "CREATE TABLE `tbdock`.`ddl_probe` (\
                  `id` bigint unsigned NOT NULL AUTO_INCREMENT,\
                  `name` varchar(32) NOT NULL,\
                  PRIMARY KEY (`id`)\
                  ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("structure".to_string()),
            transaction_id: None,
        })
        .await
        .expect("建临时表应成功");

    let alter = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "ALTER TABLE `tbdock`.`ddl_probe` ADD COLUMN `email` varchar(120) NULL AFTER `name`; \
                  ALTER TABLE `tbdock`.`ddl_probe` CHANGE COLUMN `name` `username` varchar(64) NOT NULL; \
                  ALTER TABLE `tbdock`.`ddl_probe` ADD UNIQUE INDEX `uk_username` (`username`);"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("structure".to_string()),
            transaction_id: None,
        })
        .await
        .expect("结构变更应成功");
    assert!(alter.error.is_none(), "结构变更不应报错：{:?}", alter.error);
    assert_eq!(alter.results.len(), 3);

    let altered = schema
        .table_schema(&created.connection_id, &database_name, "ddl_probe")
        .await
        .expect("改后的表结构应可读取");
    let username = altered
        .columns
        .iter()
        .find(|column| column.name == "username")
        .expect("字段应已改名");
    assert_eq!(username.column_type, "varchar(64)");
    assert!(altered.columns.iter().any(|column| column.name == "email"));
    assert!(
        altered
            .indexes
            .iter()
            .any(|index| index.name == "uk_username" && index.unique),
        "唯一索引应已创建"
    );

    // 索引修改与删除
    let revert = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "ALTER TABLE `tbdock`.`ddl_probe` DROP INDEX `uk_username`; \
                  ALTER TABLE `tbdock`.`ddl_probe` DROP COLUMN `email`;"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("structure".to_string()),
            transaction_id: None,
        })
        .await
        .expect("回退结构应成功");
    assert!(revert.error.is_none());

    let reverted = schema
        .table_schema(&created.connection_id, &database_name, "ddl_probe")
        .await
        .expect("结构应可读取");
    assert_eq!(reverted.columns.len(), 2);
    assert!(reverted
        .indexes
        .iter()
        .all(|index| index.name != "uk_username"));

    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "DROP TABLE `tbdock`.`ddl_probe`".to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("structure".to_string()),
            transaction_id: None,
        })
        .await
        .expect("删除临时表应成功");

    // 6.3.2 建库语句
    let database_ddl = schema
        .database_ddl(&created.connection_id, &database_name)
        .await
        .expect("建库语句应可读取");
    assert!(
        database_ddl.ddl.to_uppercase().contains("CREATE DATABASE"),
        "应返回 CREATE DATABASE 语句"
    );

    // 6.4 行编辑使用的语句形态：UPDATE/DELETE 带主键条件与 LIMIT 1
    let update = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "UPDATE `tbdock`.`users` SET `email` = 'alice+edited@example.com' WHERE `id` = 1 LIMIT 1"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("行更新应成功");
    assert_eq!(update.results[0].affected_rows, Some(1));

    let revert = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "UPDATE `tbdock`.`users` SET `email` = 'alice@example.com' WHERE `id` = 1 LIMIT 1"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("还原应成功");
    assert_eq!(revert.results[0].affected_rows, Some(1));

    let insert_then_delete = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "INSERT INTO event_log (level, message) VALUES ('debug', 'edit-delete-test')"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("插入测试行应成功");
    assert_eq!(insert_then_delete.results[0].affected_rows, Some(1));

    let delete = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "DELETE FROM `tbdock`.`event_log` WHERE `message` = 'edit-delete-test' LIMIT 1"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("删除测试行应成功");
    assert_eq!(delete.results[0].affected_rows, Some(1));

    // 6.4.1 新增行使用的语句形态：只列指定字段，自增主键交给数据库
    let inserted = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "INSERT INTO `tbdock`.`users` (`username`, `email`, `status`, `balance`) \
                  VALUES ('it-insert', 'it@example.com', 1, 0.0000)"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("新增行应成功");
    assert_eq!(inserted.results[0].affected_rows, Some(1));
    assert!(
        inserted.results[0].last_insert_id.is_some(),
        "新增后应返回自增主键"
    );

    let cleanup = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "DELETE FROM `tbdock`.`users` WHERE `username` = 'it-insert' LIMIT 1".to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("清理测试行应成功");
    assert_eq!(cleanup.results[0].affected_rows, Some(1));

    // 6.5 表数据分页：主键排序、总数、类型保真
    let first_page = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: Some(1),
            page_size: Some(2),
            order_by: None,
            order_desc: None,
            filter: None,
        })
        .await
        .expect("读取表数据应成功");
    println!(
        "table page1 total={} rows={} pk={:?} columns={}",
        first_page.total_rows,
        first_page.rows.len(),
        first_page.primary_key,
        first_page.columns.len()
    );
    assert_eq!(first_page.total_rows, 3);
    assert_eq!(first_page.rows.len(), 2);
    assert_eq!(first_page.primary_key, vec!["id".to_string()]);
    assert_eq!(first_page.columns.len(), 7);
    // 无 orderBy 时按主键升序，翻页才稳定
    assert_eq!(first_page.rows[0][0].value.as_deref(), Some("1"));
    // DECIMAL 仍按字符串返回，4 位小数不丢
    assert_eq!(first_page.rows[0][4].value.as_deref(), Some("1234.5600"));

    let second_page = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: Some(2),
            page_size: Some(2),
            order_by: None,
            order_desc: None,
            filter: None,
        })
        .await
        .expect("第二页应可读取");
    assert_eq!(second_page.rows.len(), 1);
    assert_eq!(second_page.rows[0][0].value.as_deref(), Some("3"));

    // 6.6 筛选与排序
    let filtered = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: None,
            page_size: None,
            order_by: None,
            order_desc: None,
            filter: Some("status = 0".to_string()),
        })
        .await
        .expect("带筛选的查询应成功");
    assert_eq!(filtered.total_rows, 1);
    assert_eq!(filtered.rows.len(), 1);
    assert_eq!(filtered.rows[0][1].value.as_deref(), Some("carol"));

    let sorted = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: None,
            page_size: None,
            order_by: Some("username".to_string()),
            order_desc: Some(true),
            filter: None,
        })
        .await
        .expect("排序查询应成功");
    assert_eq!(sorted.rows[0][1].value.as_deref(), Some("carol"));

    let bad_sort = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: None,
            page_size: None,
            order_by: Some("no_such_column".to_string()),
            order_desc: Some(false),
            filter: None,
        })
        .await;
    assert!(bad_sort.is_err(), "不存在的排序列应被拒绝");

    let bad_filter = table
        .data(TableDataRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            page: None,
            page_size: None,
            order_by: None,
            order_desc: None,
            filter: Some("id = 1; DROP TABLE users".to_string()),
        })
        .await;
    assert!(bad_filter.is_err(), "带分号的筛选应被拒绝");

    // 6.7 导出：CSV 带 BOM 与表头、JSON 为数组
    let csv_path = dir.join("users.csv");
    let csv_export = table
        .export(ExportTableRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            format: ExportFormat::Csv,
            path: csv_path.to_string_lossy().to_string(),
            filter: None,
            order_by: None,
            order_desc: None,
        })
        .await
        .expect("CSV 导出应成功");
    assert_eq!(csv_export.rows_written, 3);

    let csv_content = std::fs::read_to_string(&csv_path).expect("CSV 应可读取");
    assert!(csv_content.starts_with('\u{feff}'), "CSV 应带 UTF-8 BOM");
    assert!(csv_content.contains("id,username,email,status,balance,profile,created_at"));
    assert_eq!(csv_content.lines().count(), 4, "1 行表头 + 3 行数据");
    assert!(csv_content.contains("1234.5600"), "DECIMAL 不得丢精度");

    let json_path = dir.join("users.json");
    let json_export = table
        .export(ExportTableRequest {
            connection_id: created.connection_id.clone(),
            database_name: database_name.clone(),
            table_name: "users".to_string(),
            format: ExportFormat::Json,
            path: json_path.to_string_lossy().to_string(),
            filter: Some("status = 1".to_string()),
            order_by: None,
            order_desc: None,
        })
        .await
        .expect("JSON 导出应成功");
    assert_eq!(json_export.rows_written, 2);

    let json_content = std::fs::read_to_string(&json_path).expect("JSON 应可读取");
    let parsed: serde_json::Value = serde_json::from_str(&json_content).expect("JSON 应可解析");
    let items = parsed.as_array().expect("应导出为数组");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["username"], "alice");
    assert!(items[1]["profile"].is_null(), "NULL 应导出为 null");

    // 6.8 查询结果导出：只允许只读语句
    let query_csv = dir.join("query-result.csv");
    let query_export = query
        .export_result(ExportQueryRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT id, username, balance FROM users ORDER BY id".to_string(),
            format: ExportFormat::Csv,
            path: query_csv.to_string_lossy().to_string(),
            max_rows: None,
        })
        .await
        .expect("查询结果导出应成功");
    assert_eq!(query_export.rows_written, 3);

    let query_csv_content = std::fs::read_to_string(&query_csv).expect("CSV 应可读取");
    assert!(query_csv_content.contains("id,username,balance"));
    assert_eq!(query_csv_content.lines().count(), 4);
    assert!(query_csv_content.contains("1234.5600"));

    let rejected = query
        .export_result(ExportQueryRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "UPDATE users SET status = 1".to_string(),
            format: ExportFormat::Csv,
            path: dir
                .join("should-not-exist.csv")
                .to_string_lossy()
                .to_string(),
            max_rows: None,
        })
        .await;
    assert!(rejected.is_err(), "写语句不应允许导出");

    // 6.8.1 批量执行：整体事务，任一语句失败则全部回滚
    let failed_batch = query
        .execute_batch(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "UPDATE `tbdock`.`users` SET `status` = 0 WHERE `id` = 1 LIMIT 1; \
                  UPDATE `tbdock`.`users` SET `no_such_column` = 1 WHERE `id` = 1 LIMIT 1"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("批量执行应返回响应");
    assert!(failed_batch.rolled_back, "失败时应回滚");
    assert_eq!(failed_batch.results.len(), 1);
    assert!(failed_batch.error.is_some());

    let status_after = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT status FROM users WHERE id = 1".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("查询应成功");
    assert_eq!(
        status_after.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("1"),
        "失败批次的第一条语句应被回滚"
    );

    let ok_batch = query
        .execute_batch(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "UPDATE `tbdock`.`users` SET `status` = 0 WHERE `id` = 1 LIMIT 1; \
                  UPDATE `tbdock`.`users` SET `status` = 1 WHERE `id` = 1 LIMIT 1"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: None,
        })
        .await
        .expect("批量执行应成功");
    assert!(!ok_batch.rolled_back);
    assert!(ok_batch.error.is_none());
    assert_eq!(ok_batch.results.len(), 2);

    // 6.9 事务：未提交对其他会话不可见，回滚后不保留；提交后保留
    let transaction = query
        .begin_transaction(BeginTransactionRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
        })
        .await
        .expect("开启事务应成功");

    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "INSERT INTO event_log (level, message) VALUES ('debug', 'tx-rollback')"
                .to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: Some(transaction.transaction_id.clone()),
        })
        .await
        .expect("事务内写入应成功");

    let outside = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT COUNT(*) FROM event_log WHERE message = 'tx-rollback'".to_string(),
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
        "未提交的数据对其他会话不可见"
    );

    let rolled_back = query
        .rollback_transaction(TransactionRequest {
            connection_id: created.connection_id.clone(),
            transaction_id: transaction.transaction_id.clone(),
        })
        .await
        .expect("回滚应成功");
    assert!(rolled_back.success);

    let after_rollback = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT COUNT(*) FROM event_log WHERE message = 'tx-rollback'".to_string(),
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
        "回滚后数据应不存在"
    );

    let committed_tx = query
        .begin_transaction(BeginTransactionRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
        })
        .await
        .expect("开启第二个事务应成功");
    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "INSERT INTO event_log (level, message) VALUES ('debug', 'tx-commit')".to_string(),
            max_rows: None,
            task_id: None,
            source_type: Some("table".to_string()),
            transaction_id: Some(committed_tx.transaction_id.clone()),
        })
        .await
        .expect("事务内写入应成功");
    query
        .commit_transaction(TransactionRequest {
            connection_id: created.connection_id.clone(),
            transaction_id: committed_tx.transaction_id.clone(),
        })
        .await
        .expect("提交应成功");

    let after_commit = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT COUNT(*) FROM event_log WHERE message = 'tx-commit'".to_string(),
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
        "提交后数据应可见"
    );

    // 已结束的事务不能复用
    let stale = query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "SELECT 1".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: Some(committed_tx.transaction_id.clone()),
        })
        .await;
    assert!(stale.is_err(), "已提交的事务不应可复用");

    // 清理提交的数据
    query
        .execute(ExecuteSqlRequest {
            connection_id: created.connection_id.clone(),
            database_name: Some(database_name.clone()),
            sql: "DELETE FROM event_log WHERE message = 'tx-commit'".to_string(),
            max_rows: None,
            task_id: None,
            source_type: None,
            transaction_id: None,
        })
        .await
        .expect("清理事务测试数据应成功");

    // 7. 编辑连接（不传密码应保留凭据）
    connections
        .update(UpdateConnectionRequest {
            connection_id: created.connection_id.clone(),
            name: "IT-本地 MySQL（改）".to_string(),
            host: env("DATADOCK_MYSQL_HOST"),
            port,
            username: env("DATADOCK_MYSQL_USER"),
            default_database: Some(database_name.clone()),
            charset: None,
            connect_timeout_ms: None,
            new_password: None,
            options: None,
        })
        .await
        .expect("更新应成功");

    // 更新后运行时连接应被断开，重新连接仍应使用原密码
    assert!(connections
        .status(&created.connection_id)
        .await
        .status
        .eq("disconnected"));
    connections
        .connect(&created.connection_id)
        .await
        .expect("使用保留的凭据应可重新连接");

    // 8. 断开与删除
    connections
        .disconnect(&created.connection_id)
        .await
        .expect("断开应成功");
    connections
        .delete(&created.connection_id)
        .await
        .expect("删除应成功");
    assert!(connections.list().await.expect("列表应可读取").is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
