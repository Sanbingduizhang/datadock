//! MySQL 驱动能力验证（PoC）。
//!
//! 默认 `#[ignore]`，需要真实 MySQL 环境显式运行：
//!
//! ```bash
//! DATADOCK_MYSQL_HOST=127.0.0.1 DATADOCK_MYSQL_PORT=3306 \
//! DATADOCK_MYSQL_USER=root DATADOCK_MYSQL_PASSWORD=*** \
//! DATADOCK_MYSQL_DB=tbdock \
//! cargo test --test mysql_poc -- --ignored --nocapture --test-threads=1
//! ```
//!
//! 验证结论记录在 `docs/技术决策记录.md`。
//! 测试只在 `DATADOCK_MYSQL_DB` 指定的库内创建 `poc_*` 临时表，并在结束时删除。

use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::types::BigDecimal;
use sqlx::{Column, MySqlPool, Row, TypeInfo};

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn connect_options() -> MySqlConnectOptions {
    let host = env("DATADOCK_MYSQL_HOST").expect("缺少 DATADOCK_MYSQL_HOST");
    let user = env("DATADOCK_MYSQL_USER").expect("缺少 DATADOCK_MYSQL_USER");
    let password = env("DATADOCK_MYSQL_PASSWORD").expect("缺少 DATADOCK_MYSQL_PASSWORD");
    let database = env("DATADOCK_MYSQL_DB").expect("缺少 DATADOCK_MYSQL_DB");
    let port: u16 = env("DATADOCK_MYSQL_PORT")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3306);

    MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&user)
        .password(&password)
        .database(&database)
        .charset("utf8mb4")
}

async fn pool() -> MySqlPool {
    MySqlPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(connect_options())
        .await
        .expect("连接 MySQL 失败")
}

async fn drop_table(pool: &MySqlPool, table: &str) {
    let statement = format!("DROP TABLE IF EXISTS `{table}`");
    let _ = sqlx::raw_sql(&statement).execute(pool).await;
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_basic_and_auth() {
    let pool = pool().await;

    let version: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(&pool)
        .await
        .expect("查询版本失败");
    let connection_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
        .fetch_one(&pool)
        .await
        .expect("查询连接 ID 失败");
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&pool)
        .await
        .expect("查询当前库失败");

    println!("server_version={version} connection_id={connection_id} database={database}");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_multi_statement_behaviour() {
    let pool = pool().await;

    // 1) 预处理语句路径：多语句通常会失败（协议限制）
    match sqlx::query("SELECT 1 AS a; SELECT 2 AS b;")
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => println!("prepared_multi: ok rows={}", rows.len()),
        Err(error) => println!("prepared_multi: err={error}"),
    }

    // 2) 原始 SQL 路径
    match sqlx::raw_sql("SELECT 1 AS a; SELECT 2 AS b;")
        .execute(&pool)
        .await
    {
        Ok(result) => println!(
            "raw_sql_multi: ok rows_affected={} last_insert_id={}",
            result.rows_affected(),
            result.last_insert_id()
        ),
        Err(error) => println!("raw_sql_multi: err={error}"),
    }

    // 3) 客户端逐条执行（DataDock 计划采用的方案）
    let first: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("第一条失败");
    let second: i64 = sqlx::query_scalar("SELECT 2")
        .fetch_one(&pool)
        .await
        .expect("第二条失败");
    println!("sequential: {first},{second}");
    assert_eq!((first, second), (1, 2));
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_data_types() {
    let pool = pool().await;
    drop_table(&pool, "poc_types").await;

    sqlx::raw_sql(
        "CREATE TABLE `poc_types` (
            id BIGINT UNSIGNED PRIMARY KEY,
            amount DECIMAL(30,10),
            title VARCHAR(64),
            payload JSON,
            raw BLOB,
            created_at DATETIME(3),
            flag TINYINT(1),
            note TEXT
        )",
    )
    .execute(&pool)
    .await
    .expect("建表失败");

    sqlx::query("INSERT INTO `poc_types` VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(18_446_744_073_709_551_615_u64)
        .bind("12345678901234567890.1234567890")
        .bind("中文标题")
        .bind(serde_json::json!({ "k": "v", "n": 1 }))
        .bind(vec![0_u8, 1, 2, 255])
        .bind("2026-11-18 12:00:00.123")
        .bind(1_i8)
        .bind("长文本")
        .execute(&pool)
        .await
        .expect("插入失败");

    let row = sqlx::query("SELECT * FROM `poc_types`")
        .fetch_one(&pool)
        .await
        .expect("查询失败");

    for column in row.columns() {
        println!(
            "column={} type={}",
            column.name(),
            column.type_info().name()
        );
    }

    let big: u64 = row.try_get("id").expect("BIGINT UNSIGNED 读取失败");
    let amount: BigDecimal = row.try_get("amount").expect("DECIMAL 读取失败");
    let title: String = row.try_get("title").expect("VARCHAR 读取失败");
    let payload: serde_json::Value = row.try_get("payload").expect("JSON 读取失败");
    let raw: Vec<u8> = row.try_get("raw").expect("BLOB 读取失败");
    let created: NaiveDateTime = row.try_get("created_at").expect("DATETIME 读取失败");
    let flag: i8 = row.try_get("flag").expect("TINYINT 读取失败");

    // f64 读法在 sqlx 中直接被拒绝，说明必须使用 BigDecimal 保精度
    let as_f64: Result<f64, _> = row.try_get("amount");
    println!(
        "decimal_as_f64 = {:?}",
        as_f64.as_ref().err().map(|error| error.to_string())
    );

    println!(
        "id={big} amount={amount} title={title} json={payload} raw={raw:?} created={created} flag={flag}"
    );

    assert_eq!(big, u64::MAX);
    assert_eq!(amount.to_string(), "12345678901234567890.1234567890");
    assert_eq!(title, "中文标题");
    assert_eq!(raw, vec![0_u8, 1, 2, 255]);

    drop_table(&pool, "poc_types").await;
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_transaction() {
    let pool = pool().await;
    drop_table(&pool, "poc_tx").await;

    sqlx::raw_sql("CREATE TABLE `poc_tx` (id INT PRIMARY KEY, v VARCHAR(16))")
        .execute(&pool)
        .await
        .expect("建表失败");

    let mut transaction = pool.begin().await.expect("开启事务失败");
    sqlx::query("INSERT INTO `poc_tx` VALUES (1, 'a')")
        .execute(&mut *transaction)
        .await
        .expect("插入失败");
    transaction.rollback().await.expect("回滚失败");

    let after_rollback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `poc_tx`")
        .fetch_one(&pool)
        .await
        .expect("统计失败");

    let mut transaction = pool.begin().await.expect("开启事务失败");
    sqlx::query("INSERT INTO `poc_tx` VALUES (2, 'b')")
        .execute(&mut *transaction)
        .await
        .expect("插入失败");
    transaction.commit().await.expect("提交失败");

    let after_commit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `poc_tx`")
        .fetch_one(&pool)
        .await
        .expect("统计失败");

    println!("after_rollback={after_rollback} after_commit={after_commit}");
    assert_eq!((after_rollback, after_commit), (0, 1));

    drop_table(&pool, "poc_tx").await;
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_streaming() {
    let pool = pool().await;
    drop_table(&pool, "poc_stream").await;

    sqlx::raw_sql("CREATE TABLE `poc_stream` (id INT PRIMARY KEY AUTO_INCREMENT, v VARCHAR(32))")
        .execute(&pool)
        .await
        .expect("建表失败");

    for index in 0..300 {
        sqlx::query("INSERT INTO `poc_stream` (v) VALUES (?)")
            .bind(format!("row-{index}"))
            .execute(&pool)
            .await
            .expect("插入失败");
    }

    let started = Instant::now();
    let mut rows = sqlx::query("SELECT id, v FROM `poc_stream` ORDER BY id").fetch(&pool);
    let mut count = 0_usize;
    while let Some(row) = rows.try_next().await.expect("流式读取失败") {
        let id: i32 = row.try_get("id").expect("读取 id 失败");
        let _: String = row.try_get("v").expect("读取 v 失败");
        assert!(id > 0);
        count += 1;
    }

    println!("streamed_rows={count} elapsed={:?}", started.elapsed());
    assert_eq!(count, 300);

    drop_table(&pool, "poc_stream").await;
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
async fn poc_cancel_via_kill_query() {
    let pool = pool().await;

    let mut worker = pool.acquire().await.expect("获取连接失败");
    let connection_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
        .fetch_one(&mut *worker)
        .await
        .expect("查询连接 ID 失败");
    println!("worker_connection_id={connection_id}");

    let cancel_pool = pool.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let statement = format!("KILL QUERY {connection_id}");
        match sqlx::raw_sql(&statement).execute(&cancel_pool).await {
            Ok(result) => println!("kill_query ok rows_affected={}", result.rows_affected()),
            Err(error) => println!("kill_query err={error}"),
        }
    });

    let started = Instant::now();
    let outcome = sqlx::query("SELECT SLEEP(10)")
        .fetch_one(&mut *worker)
        .await;
    let elapsed = started.elapsed();

    let _ = canceller.await;

    match &outcome {
        Ok(row) => println!(
            "sleep_result=ok value={:?} elapsed={:?}",
            row.try_get::<i8, _>("SLEEP(10)"),
            elapsed
        ),
        Err(error) => println!("sleep_result=err {error} elapsed={:?}", elapsed),
    }

    assert!(elapsed < Duration::from_secs(5), "KILL QUERY 未能中断查询");
}

#[tokio::test]
#[ignore = "需要真实 MySQL 环境"]
#[allow(clippy::type_complexity)]
async fn poc_metadata() {
    let pool = pool().await;
    drop_table(&pool, "poc_meta").await;

    sqlx::raw_sql(
        "CREATE TABLE `poc_meta` (
            id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT COMMENT '主键',
            name VARCHAR(64) NOT NULL DEFAULT '',
            amount DECIMAL(12,2) NULL,
            PRIMARY KEY (id),
            UNIQUE KEY `uk_name` (`name`),
            KEY `idx_amount` (`amount`)
        ) ENGINE=InnoDB COMMENT='元数据验证表'",
    )
    .execute(&pool)
    .await
    .expect("建表失败");

    let databases: Vec<String> = sqlx::query_scalar(
        "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME",
    )
    .fetch_all(&pool)
    .await
    .expect("数据库列表失败");
    println!("databases={databases:?}");

    let tables: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT TABLE_NAME, CAST(TABLE_TYPE AS CHAR), CAST(ENGINE AS CHAR), CAST(TABLE_COMMENT AS CHAR)
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA = DATABASE()
         ORDER BY TABLE_NAME",
    )
    .fetch_all(&pool)
    .await
    .expect("表列表失败");
    println!("tables={tables:?}");

    let columns: Vec<(
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT CAST(COLUMN_NAME AS CHAR), CAST(COLUMN_TYPE AS CHAR), CAST(IS_NULLABLE AS CHAR),
                    CAST(COLUMN_DEFAULT AS CHAR), CAST(COLUMN_KEY AS CHAR), CAST(EXTRA AS CHAR),
                    CAST(COLUMN_COMMENT AS CHAR)
             FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'poc_meta'
             ORDER BY ORDINAL_POSITION",
    )
    .fetch_all(&pool)
    .await
    .expect("字段查询失败");
    println!("columns={columns:?}");

    let indexes: Vec<(String, i64, u64, String, String)> = sqlx::query_as(
        "SELECT CAST(INDEX_NAME AS CHAR), NON_UNIQUE, SEQ_IN_INDEX, CAST(COLUMN_NAME AS CHAR), CAST(INDEX_TYPE AS CHAR)
         FROM information_schema.STATISTICS
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'poc_meta'
         ORDER BY INDEX_NAME, SEQ_IN_INDEX",
    )
    .fetch_all(&pool)
    .await
    .expect("索引查询失败");
    println!("indexes={indexes:?}");

    let ddl: Vec<(String, String)> = sqlx::query_as("SHOW CREATE TABLE `poc_meta`")
        .fetch_all(&pool)
        .await
        .expect("SHOW CREATE TABLE 失败");
    println!(
        "ddl_len={}",
        ddl.first().map(|(_, ddl)| ddl.len()).unwrap_or(0)
    );

    assert!(tables.iter().any(|(name, ..)| name == "poc_meta"));
    assert_eq!(columns.len(), 3);
    assert!(indexes.len() >= 3);

    drop_table(&pool, "poc_meta").await;
}
