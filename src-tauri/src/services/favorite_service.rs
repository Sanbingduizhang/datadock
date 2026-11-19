//! SQL 收藏业务。

use crate::database::Database;
use crate::errors::AppError;
use crate::models::favorite::{NewSqlFavorite, SqlFavorite};
use crate::repositories::favorite_repository;

pub const MAX_NAME_LEN: usize = 120;
pub const MAX_SQL_LEN: usize = 200_000;

pub struct FavoriteService {
    database: Database,
}

impl FavoriteService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list(&self) -> Result<Vec<SqlFavorite>, AppError> {
        favorite_repository::list(self.database.pool()).await
    }

    pub async fn create(&self, input: NewSqlFavorite) -> Result<String, AppError> {
        let input = validate(input)?;
        favorite_repository::insert(self.database.pool(), &input).await
    }

    pub async fn update(&self, favorite_id: &str, input: NewSqlFavorite) -> Result<bool, AppError> {
        let input = validate(input)?;
        favorite_repository::update(self.database.pool(), favorite_id, &input).await
    }

    pub async fn delete(&self, favorite_id: &str) -> Result<bool, AppError> {
        favorite_repository::delete(self.database.pool(), favorite_id).await
    }
}

fn validate(mut input: NewSqlFavorite) -> Result<NewSqlFavorite, AppError> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::validation("收藏名称不能为空"));
    }
    if input.name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::validation(format!(
            "收藏名称过长，最多 {MAX_NAME_LEN} 个字符"
        )));
    }
    if input.sql_text.trim().is_empty() {
        return Err(AppError::validation("SQL 内容不能为空"));
    }
    if input.sql_text.len() > MAX_SQL_LEN {
        return Err(AppError::validation("SQL 内容过长，无法收藏"));
    }

    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, sql: &str) -> NewSqlFavorite {
        NewSqlFavorite {
            name: name.to_string(),
            connection_id: None,
            database_name: None,
            sql_text: sql.to_string(),
            category: None,
            remark: None,
        }
    }

    #[test]
    fn rejects_blank_name_and_sql() {
        assert!(validate(input("  ", "SELECT 1")).is_err());
        assert!(validate(input("名字", "   ")).is_err());
    }

    #[test]
    fn trims_name() {
        let result = validate(input("  常用查询  ", "SELECT 1")).expect("应通过");
        assert_eq!(result.name, "常用查询");
    }

    #[test]
    fn rejects_too_long_name() {
        let name = "a".repeat(MAX_NAME_LEN + 1);
        assert!(validate(input(&name, "SELECT 1")).is_err());
    }
}

#[cfg(test)]
mod store_tests {
    use super::*;
    use crate::database::Database;

    async fn service() -> FavoriteService {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        FavoriteService::new(database)
    }

    fn sample(name: &str) -> NewSqlFavorite {
        NewSqlFavorite {
            name: name.to_string(),
            connection_id: Some("conn-1".to_string()),
            database_name: Some("tbdock".to_string()),
            sql_text: "SELECT * FROM users".to_string(),
            category: None,
            remark: None,
        }
    }

    #[tokio::test]
    async fn create_list_update_delete() {
        let service = service().await;

        let id = service
            .create(sample("用户查询"))
            .await
            .expect("创建应成功");
        let items = service.list().await.expect("列表应可读取");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "用户查询");
        assert_eq!(items[0].sql_text, "SELECT * FROM users");

        assert!(service
            .update(&id, sample("用户查询（改）"))
            .await
            .expect("更新应成功"));
        let updated = service.list().await.expect("列表应可读取");
        assert_eq!(updated[0].name, "用户查询（改）");

        assert!(service.delete(&id).await.expect("删除应成功"));
        assert!(service.list().await.expect("列表应可读取").is_empty());
    }

    #[tokio::test]
    async fn rejects_blank_name() {
        let service = service().await;
        assert!(service.create(sample("   ")).await.is_err());
    }
}
