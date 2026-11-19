//! Redis 连接与命令控制台集成测试。
//!
//! 需要本地 Redis（默认 127.0.0.1:6379），密码通过 `DATADOCK_REDIS_PASSWORD` 传入。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use datadock_lib::core::{ConnectionManager, PgManager, RedisManager};
use datadock_lib::credentials::CredentialStore;
use datadock_lib::database::Database;
use datadock_lib::errors::AppError;
use datadock_lib::models::connection::{CreateConnectionRequest, TestConnectionRequest};
use datadock_lib::models::redis::{
    RedisKeyType, RedisKeyValue, RedisKeyValueRequest, RedisScanRequest,
};
use datadock_lib::services::{ConnectionService, RedisService};

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

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("缺少环境变量 {name}"))
}

async fn services() -> (ConnectionService, RedisService, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("datadock-redis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("临时目录应可创建");
    let path = dir.join("test.db");

    let database = Database::connect(&path).await.expect("本地库应可创建");
    database.migrate().await.expect("迁移应成功");

    let credentials: Arc<dyn CredentialStore> = Arc::new(InMemoryCredentialStore::default());
    let manager = Arc::new(ConnectionManager::new());
    let redis_manager = Arc::new(RedisManager::new());

    (
        ConnectionService::new(
            database.clone(),
            credentials,
            manager,
            redis_manager.clone(),
            Arc::new(PgManager::new()),
        ),
        RedisService::new(redis_manager),
        dir,
    )
}

#[tokio::test]
#[ignore = "需要本地 Redis 与密码"]
async fn redis_console_round_trip() {
    let (connections, redis, dir) = services().await;

    let host = std::env::var("DATADOCK_REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("DATADOCK_REDIS_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(6379);
    let password = env("DATADOCK_REDIS_PASSWORD");

    // 1. 测试连接
    let tested = connections
        .test(TestConnectionRequest {
            source_type: "redis".to_string(),
            host: host.clone(),
            port,
            username: String::new(),
            password: password.clone(),
            default_database: Some("0".to_string()),
            charset: None,
            connect_timeout_ms: Some(5_000),
            options: None,
        })
        .await
        .expect("测试连接应返回结果");
    println!(
        "redis test: success={} version={:?}",
        tested.success, tested.server_version
    );
    assert!(tested.success, "Redis 应连接成功：{}", tested.message);

    // 2. 保存配置并连接
    let created = connections
        .create(CreateConnectionRequest {
            name: "IT-Redis".to_string(),
            source_type: "redis".to_string(),
            host: host.clone(),
            port,
            username: String::new(),
            password: password.clone(),
            default_database: Some("0".to_string()),
            charset: None,
            connect_timeout_ms: Some(5_000),
            options: None,
        })
        .await
        .expect("创建连接应成功");

    let connected = connections
        .connect(&created.connection_id)
        .await
        .expect("连接 Redis 应成功");
    assert_eq!(connected.status, "connected");
    println!(
        "redis connected: version={:?} db={:?}",
        connected.server_version, connected.current_database
    );

    // 3. 基础命令
    let ping = redis
        .execute(&created.connection_id, "PING")
        .await
        .expect("PING 应成功");
    assert_eq!(
        ping.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("PONG")
    );

    let key = format!("datadock:test:{}", std::process::id());
    redis
        .execute(
            &created.connection_id,
            &format!("SET {key} \"hello world\""),
        )
        .await
        .expect("SET 应成功");

    let got = redis
        .execute(&created.connection_id, &format!("GET {key}"))
        .await
        .expect("GET 应成功");
    assert_eq!(
        got.results[0]
            .result_set
            .as_ref()
            .and_then(|set| set.rows[0][0].value.as_deref()),
        Some("hello world")
    );

    // 4. 哈希：两列键值对
    let hash_key = format!("{key}:hash");
    redis
        .execute(
            &created.connection_id,
            &format!("HSET {hash_key} name alice city shanghai"),
        )
        .await
        .expect("HSET 应成功");
    let hash = redis
        .execute(&created.connection_id, &format!("HGETALL {hash_key}"))
        .await
        .expect("HGETALL 应成功");
    let hash_set = hash.results[0].result_set.as_ref().expect("应有结果集");
    assert_eq!(hash_set.columns.len(), 2);
    assert_eq!(hash_set.row_count, 2);
    println!(
        "hash rows: {:?}",
        hash_set
            .rows
            .iter()
            .map(|row| row
                .iter()
                .map(|cell| cell.value.clone().unwrap_or_default())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );

    // 5. 键浏览：SCAN + 值读取（覆盖 string / hash / list / set / zset 五种类型）
    let pid = std::process::id();
    let str_key = format!("datadock:scan:str:{pid}");
    let hash_key2 = format!("datadock:scan:hash:{pid}");
    let list_key = format!("datadock:scan:list:{pid}");
    let set_key = format!("datadock:scan:set:{pid}");
    let zset_key = format!("datadock:scan:zset:{pid}");

    redis
        .execute(
            &created.connection_id,
            &format!("SET {str_key} 'scan-test'"),
        )
        .await
        .expect("SET");
    redis
        .execute(
            &created.connection_id,
            &format!("HSET {hash_key2} f1 v1 f2 v2"),
        )
        .await
        .expect("HSET");
    redis
        .execute(&created.connection_id, &format!("RPUSH {list_key} a b c"))
        .await
        .expect("RPUSH");
    redis
        .execute(&created.connection_id, &format!("SADD {set_key} x y z"))
        .await
        .expect("SADD");
    redis
        .execute(
            &created.connection_id,
            &format!("ZADD {zset_key} 1.5 alpha 2.5 beta"),
        )
        .await
        .expect("ZADD");

    // SCAN 匹配扫描
    let scan = redis
        .scan_keys(RedisScanRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            pattern: Some("datadock:scan:*".to_string()),
            count: Some(100),
            include_system_keys: false,
        })
        .await
        .expect("SCAN 应成功");
    println!("scan found {} keys", scan.keys.len());
    assert!(
        scan.keys.len() >= 5,
        "应至少找到 5 个键，实际 {}",
        scan.keys.len()
    );

    // 按类型验证
    let type_map: std::collections::HashMap<String, RedisKeyType> = scan
        .keys
        .iter()
        .map(|k| (k.name.clone(), k.key_type))
        .collect();

    assert_eq!(
        type_map.get(&str_key),
        Some(&RedisKeyType::String),
        "str_key 应为 string 类型"
    );
    assert_eq!(
        type_map.get(&hash_key2),
        Some(&RedisKeyType::Hash),
        "hash_key 应为 hash 类型"
    );
    assert_eq!(
        type_map.get(&list_key),
        Some(&RedisKeyType::List),
        "list_key 应为 list 类型"
    );
    assert_eq!(
        type_map.get(&set_key),
        Some(&RedisKeyType::Set),
        "set_key 应为 set 类型"
    );
    assert_eq!(
        type_map.get(&zset_key),
        Some(&RedisKeyType::Zset),
        "zset_key 应为 zset 类型"
    );
    println!("type check: all 5 key types verified");

    // 值读取：string
    let str_val = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: str_key.clone(),
            max_items: Some(100),
        })
        .await
        .expect("GET 值应成功");
    assert!(
        matches!(str_val.value, RedisKeyValue::String { value: Some(ref v) } if v == "scan-test"),
        "string 值应为 'scan-test'，实际 {:?}",
        str_val.value
    );
    assert_eq!(str_val.key_type, RedisKeyType::String);

    // 值读取：hash
    let hash_val = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: hash_key2.clone(),
            max_items: Some(100),
        })
        .await
        .expect("HGETALL 值应成功");
    match &hash_val.value {
        RedisKeyValue::Hash { fields } => {
            assert_eq!(fields.len(), 2, "hash 应有 2 个字段");
            let f1 = fields
                .iter()
                .find(|f| f.field == "f1")
                .expect("应有 f1 字段");
            assert_eq!(f1.value, "v1");
        }
        other => panic!("hash 值类型错误：{:?}", other),
    }

    // 值读取：list
    let list_val = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: list_key.clone(),
            max_items: Some(100),
        })
        .await
        .expect("LRANGE 值应成功");
    match &list_val.value {
        RedisKeyValue::List { items } => {
            assert_eq!(items.len(), 3, "list 应有 3 个元素");
            assert_eq!(items[0].value, "a");
            assert_eq!(items[1].value, "b");
        }
        other => panic!("list 值类型错误：{:?}", other),
    }

    // 值读取：set
    let set_val = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: set_key.clone(),
            max_items: Some(100),
        })
        .await
        .expect("SMEMBERS 值应成功");
    match &set_val.value {
        RedisKeyValue::Set { members } => {
            assert_eq!(members.len(), 3, "set 应有 3 个成员");
        }
        other => panic!("set 值类型错误：{:?}", other),
    }

    // 值读取：zset
    let zset_val = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: zset_key.clone(),
            max_items: Some(100),
        })
        .await
        .expect("ZRANGE 值应成功");
    match &zset_val.value {
        RedisKeyValue::Zset { entries } => {
            assert_eq!(entries.len(), 2, "zset 应有 2 个条目");
            assert_eq!(entries[0].member, "alpha");
            assert!((entries[0].score - 1.5).abs() < 1e-9);
        }
        other => panic!("zset 值类型错误：{:?}", other),
    }

    println!("value check: all 5 key types read correctly");

    // TTL 验证（-1 = 永不过期）
    let str_info = scan
        .keys
        .iter()
        .find(|k| k.name == str_key)
        .expect("应有 str_key");
    assert_eq!(str_info.ttl_seconds, -1, "未设置过期时间的键 TTL 应为 -1");

    // 设置 TTL 后验证
    redis
        .execute(&created.connection_id, &format!("EXPIRE {str_key} 3600"))
        .await
        .expect("EXPIRE 应成功");
    let str_val_after_expire = redis
        .get_key_value(RedisKeyValueRequest {
            connection_id: created.connection_id.clone(),
            database: None,
            key_name: str_key.clone(),
            max_items: Some(100),
        })
        .await
        .expect("TTL 值应成功");
    assert!(
        str_val_after_expire.ttl_seconds > 0 && str_val_after_expire.ttl_seconds <= 3600,
        "TTL 应在 (0, 3600] 范围内，实际 {}",
        str_val_after_expire.ttl_seconds
    );
    println!(
        "TTL check: {}s after EXPIRE 3600",
        str_val_after_expire.ttl_seconds
    );

    // 6. 库列表
    let databases = redis
        .list_databases(&created.connection_id)
        .await
        .expect("库列表应可读取");
    assert_eq!(databases.len(), 1);
    assert_eq!(databases[0].name, "db0");

    // 7. 清理扫描的测试键
    redis
        .execute(
            &created.connection_id,
            &format!("DEL {key} {hash_key} {str_key} {hash_key2} {list_key} {set_key} {zset_key}"),
        )
        .await
        .expect("DEL 应成功");

    connections
        .disconnect(&created.connection_id)
        .await
        .expect("断开应成功");
    assert_eq!(connections.list().await.expect("列表应可读取").len(), 1);
    connections
        .delete(&created.connection_id)
        .await
        .expect("删除应成功");

    let _ = std::fs::remove_dir_all(&dir);
}
