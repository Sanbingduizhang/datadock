//! 应用设置持久化。

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::utils::time::{now_ms, to_db_time};

const SELECT_ALL: &str = "SELECT key, value FROM app_settings ORDER BY key";

const UPSERT: &str = r#"
INSERT INTO app_settings (key, value, value_type, updated_at)
VALUES (?, ?, ?, ?)
ON CONFLICT(key) DO UPDATE SET
    value = excluded.value,
    value_type = excluded.value_type,
    updated_at = excluded.updated_at
"#;

/// 读取全部设置键值。JSON 类型的设置也以字符串形式返回，由上层解析。
pub async fn load_map(pool: &SqlitePool) -> Result<HashMap<String, String>, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(SELECT_ALL)
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取应用设置失败：{error}")))?;

    Ok(rows.into_iter().collect())
}

/// 写入默认设置。使用 `INSERT OR IGNORE`，重复启动不会覆盖用户已修改的值。
pub async fn ensure_defaults(
    pool: &SqlitePool,
    defaults: &[(&str, &str, &str)],
) -> Result<(), AppError> {
    let now = to_db_time(now_ms());
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| AppError::storage(format!("开启设置事务失败：{error}")))?;

    for (key, value, value_type) in defaults {
        sqlx::query(
            "INSERT OR IGNORE INTO app_settings (key, value, value_type, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(key)
        .bind(value)
        .bind(value_type)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| AppError::storage(format!("写入默认设置失败：{error}")))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| AppError::storage(format!("提交设置事务失败：{error}")))?;

    Ok(())
}

/// 批量写入设置（事务内完成）。
pub async fn upsert_many(
    pool: &SqlitePool,
    entries: &[(&str, String, &str)],
) -> Result<(), AppError> {
    let now = to_db_time(now_ms());
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| AppError::storage(format!("开启设置事务失败：{error}")))?;

    for (key, value, value_type) in entries {
        sqlx::query(UPSERT)
            .bind(key)
            .bind(value)
            .bind(value_type)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(|error| AppError::storage(format!("更新设置失败：{error}")))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| AppError::storage(format!("提交设置事务失败：{error}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::models::settings::{AppSettings, DEFAULT_SETTINGS, KEY_FONT_SIZE, KEY_THEME};

    async fn setup() -> Database {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        database
    }

    #[tokio::test]
    async fn ensure_defaults_is_idempotent() {
        let database = setup().await;

        ensure_defaults(database.pool(), DEFAULT_SETTINGS)
            .await
            .expect("首次写入应成功");
        ensure_defaults(database.pool(), DEFAULT_SETTINGS)
            .await
            .expect("重复写入应成功");

        let stored = load_map(database.pool()).await.expect("读取应成功");
        assert_eq!(stored.len(), DEFAULT_SETTINGS.len());
        assert_eq!(stored.get(KEY_THEME).map(String::as_str), Some("system"));
    }

    #[tokio::test]
    async fn defaults_do_not_override_user_values() {
        let database = setup().await;
        ensure_defaults(database.pool(), DEFAULT_SETTINGS)
            .await
            .expect("写入默认值应成功");

        upsert_many(
            database.pool(),
            &[(KEY_FONT_SIZE, "16".to_string(), "number")],
        )
        .await
        .expect("更新应成功");
        ensure_defaults(database.pool(), DEFAULT_SETTINGS)
            .await
            .expect("再次写入默认值应成功");

        let stored = load_map(database.pool()).await.expect("读取应成功");
        assert_eq!(stored.get(KEY_FONT_SIZE).map(String::as_str), Some("16"));
    }

    #[tokio::test]
    async fn round_trip_settings() {
        let database = setup().await;
        let settings = AppSettings {
            appearance: crate::models::settings::AppearanceSettings {
                theme: crate::models::settings::ThemeMode::Dark,
            },
            ..AppSettings::default()
        };

        upsert_many(database.pool(), &settings.key_values())
            .await
            .expect("保存应成功");

        let stored = load_map(database.pool()).await.expect("读取应成功");
        assert_eq!(AppSettings::from_stored(&stored), settings);
    }
}
