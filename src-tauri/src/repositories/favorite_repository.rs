//! SQL 收藏持久化。

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::models::favorite::{NewSqlFavorite, SqlFavorite};
use crate::utils::time::{new_id, now_ms, to_db_time};

const SELECT_COLUMNS: &str = "id, name, connection_id, database_name, sql_text, category, remark, \
     sort_order, created_at, updated_at";

pub async fn list(pool: &SqlitePool) -> Result<Vec<SqlFavorite>, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM sql_favorites ORDER BY sort_order, created_at");

    sqlx::query_as::<_, SqlFavorite>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取 SQL 收藏失败：{error}")))
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<SqlFavorite>, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM sql_favorites WHERE id = ?");

    sqlx::query_as::<_, SqlFavorite>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取 SQL 收藏失败：{error}")))
}

pub async fn insert(pool: &SqlitePool, input: &NewSqlFavorite) -> Result<String, AppError> {
    let id = new_id();
    let now = to_db_time(now_ms());

    sqlx::query(
        "INSERT INTO sql_favorites \
         (id, name, connection_id, database_name, sql_text, category, remark, \
          sort_order, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.connection_id)
    .bind(&input.database_name)
    .bind(&input.sql_text)
    .bind(&input.category)
    .bind(&input.remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("保存 SQL 收藏失败：{error}")))?;

    Ok(id)
}

pub async fn update(pool: &SqlitePool, id: &str, input: &NewSqlFavorite) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE sql_favorites SET \
         name = ?, connection_id = ?, database_name = ?, sql_text = ?, category = ?, \
         remark = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.connection_id)
    .bind(&input.database_name)
    .bind(&input.sql_text)
    .bind(&input.category)
    .bind(&input.remark)
    .bind(to_db_time(now_ms()))
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("更新 SQL 收藏失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM sql_favorites WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("删除 SQL 收藏失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn new_favorite(name: &str) -> NewSqlFavorite {
        NewSqlFavorite {
            name: name.to_string(),
            connection_id: Some("conn-1".to_string()),
            database_name: Some("tbdock".to_string()),
            sql_text: "SELECT * FROM users".to_string(),
            category: Some("常用".to_string()),
            remark: None,
        }
    }

    async fn setup() -> Database {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        database
    }

    #[tokio::test]
    async fn insert_update_delete() {
        let database = setup().await;

        let id = insert(database.pool(), &new_favorite("用户查询"))
            .await
            .expect("插入应成功");
        let created = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");
        assert_eq!(created.name, "用户查询");
        assert_eq!(created.category.as_deref(), Some("常用"));

        let mut changed = new_favorite("用户查询（改）");
        changed.remark = Some("备注".to_string());
        assert!(update(database.pool(), &id, &changed)
            .await
            .expect("更新应成功"));

        let updated = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");
        assert_eq!(updated.name, "用户查询（改）");
        assert_eq!(updated.remark.as_deref(), Some("备注"));

        assert!(delete(database.pool(), &id).await.expect("删除应成功"));
        assert_eq!(list(database.pool()).await.expect("列表应可读取").len(), 0);
    }

    #[tokio::test]
    async fn allows_favorite_without_connection() {
        let database = setup().await;
        let mut input = new_favorite("通用模板");
        input.connection_id = None;
        input.database_name = None;

        let id = insert(database.pool(), &input).await.expect("插入应成功");
        let created = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");

        assert!(created.connection_id.is_none());
    }
}
