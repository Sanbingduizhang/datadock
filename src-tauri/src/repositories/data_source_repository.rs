//! 数据源连接配置持久化。
//!
//! 该表不保存密码；`credential_key` 指向系统凭据服务中的条目。

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::models::data_source::{DataSourceConfig, NewDataSource};
use crate::utils::time::{now_ms, to_db_time};

const SELECT_COLUMNS: &str = "id, name, source_type, host, port, username, credential_key, \
     default_database, options_json, remark, sort_order, created_at, updated_at";

pub async fn list(pool: &SqlitePool) -> Result<Vec<DataSourceConfig>, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM data_sources ORDER BY sort_order, created_at");

    sqlx::query_as::<_, DataSourceConfig>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取数据源列表失败：{error}")))
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<DataSourceConfig>, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM data_sources WHERE id = ?");

    sqlx::query_as::<_, DataSourceConfig>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取数据源失败：{error}")))
}

/// 插入连接配置。ID 由 Service 层生成，以便与系统凭据键保持一致。
pub async fn insert(
    pool: &SqlitePool,
    id: &str,
    input: &NewDataSource,
    credential_key: Option<&str>,
) -> Result<(), AppError> {
    let now = to_db_time(now_ms());

    sqlx::query(
        "INSERT INTO data_sources \
         (id, name, source_type, host, port, username, credential_key, default_database, \
          options_json, remark, sort_order, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.source_type)
    .bind(&input.host)
    .bind(i64::from(input.port))
    .bind(&input.username)
    .bind(credential_key)
    .bind(&input.default_database)
    .bind(&input.options_json)
    .bind(&input.remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("保存数据源失败：{error}")))?;

    Ok(())
}

/// 更新连接配置（不含密码）。返回是否命中记录。
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: &NewDataSource,
    credential_key: Option<&str>,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE data_sources SET \
         name = ?, host = ?, port = ?, username = ?, credential_key = ?, \
         default_database = ?, options_json = ?, remark = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.host)
    .bind(i64::from(input.port))
    .bind(&input.username)
    .bind(credential_key)
    .bind(&input.default_database)
    .bind(&input.options_json)
    .bind(&input.remark)
    .bind(to_db_time(now_ms()))
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("更新数据源失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM data_sources WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("删除数据源失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn new_source(name: &str) -> NewDataSource {
        NewDataSource {
            name: name.to_string(),
            source_type: "mysql".to_string(),
            host: "127.0.0.1".to_string(),
            port: 3306,
            username: "root".to_string(),
            default_database: Some("tbdock".to_string()),
            options_json: None,
            remark: None,
        }
    }

    async fn setup() -> Database {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        database
    }

    #[tokio::test]
    async fn insert_and_read_back() {
        let database = setup().await;

        let id = "conn-1".to_string();
        insert(
            database.pool(),
            &id,
            &new_source("本地"),
            Some("datadock/cred-1"),
        )
        .await
        .expect("插入应成功");
        let config = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");

        assert_eq!(config.name, "本地");
        assert_eq!(config.port, 3306);
        assert_eq!(config.credential_key.as_deref(), Some("datadock/cred-1"));
        assert!(config.created_at > 0);
    }

    #[tokio::test]
    async fn list_orders_by_sort_order() {
        let database = setup().await;
        let first = "conn-a".to_string();
        let second = "conn-b".to_string();
        insert(database.pool(), &first, &new_source("A"), None)
            .await
            .expect("插入应成功");
        insert(database.pool(), &second, &new_source("B"), None)
            .await
            .expect("插入应成功");

        sqlx::query("UPDATE data_sources SET sort_order = 10 WHERE id = ?")
            .bind(&first)
            .execute(database.pool())
            .await
            .expect("更新排序应成功");

        let list = list(database.pool()).await.expect("列表应可读取");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, second);
        assert_eq!(list[1].id, first);
    }

    #[tokio::test]
    async fn update_and_delete() {
        let database = setup().await;
        let id = "conn-old".to_string();
        insert(database.pool(), &id, &new_source("旧名称"), None)
            .await
            .expect("插入应成功");

        let mut changed = new_source("新名称");
        changed.port = 3307;
        assert!(
            update(database.pool(), &id, &changed, Some("datadock/cred-2"))
                .await
                .expect("更新应成功")
        );

        let config = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");
        assert_eq!(config.name, "新名称");
        assert_eq!(config.port, 3307);
        assert_eq!(config.credential_key.as_deref(), Some("datadock/cred-2"));

        assert!(delete(database.pool(), &id).await.expect("删除应成功"));
        assert!(get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .is_none());
    }

    #[tokio::test]
    async fn deleting_missing_row_returns_false() {
        let database = setup().await;

        assert!(!delete(database.pool(), "not-exist")
            .await
            .expect("删除不应报错"));
    }
}
