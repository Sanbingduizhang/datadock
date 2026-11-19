//! 应用设置业务逻辑。

use crate::database::Database;
use crate::errors::AppError;
use crate::models::settings::{AppSettings, PartialAppSettings};
use crate::repositories::settings_repository;

pub struct SettingsService {
    db: Database,
}

impl SettingsService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 读取完整设置；缺失项使用默认值。
    pub async fn load(&self) -> Result<AppSettings, AppError> {
        let stored = settings_repository::load_map(self.db.pool()).await?;
        Ok(AppSettings::from_stored(&stored))
    }

    /// 合并部分更新，校验后仅写入发生变化的键。
    pub async fn update(&self, partial: &PartialAppSettings) -> Result<AppSettings, AppError> {
        let current = self.load().await?;
        let next = current.merged(partial);
        next.validate()?;

        let current_map: std::collections::HashMap<&str, String> = current
            .key_values()
            .into_iter()
            .map(|(key, value, _)| (key, value))
            .collect();

        let changed: Vec<(&str, String, &str)> = next
            .key_values()
            .into_iter()
            .filter(|(key, value, _)| current_map.get(key) != Some(value))
            .collect();

        if changed.is_empty() {
            return Ok(current);
        }

        settings_repository::upsert_many(self.db.pool(), &changed).await?;

        tracing::debug!(
            target: "datadock::settings",
            changed = changed.len(),
            "应用设置已更新"
        );

        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::{ThemeMode, DEFAULT_SETTINGS, KEY_THEME};

    async fn service() -> SettingsService {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        settings_repository::ensure_defaults(database.pool(), DEFAULT_SETTINGS)
            .await
            .expect("写入默认值应成功");

        SettingsService::new(database)
    }

    #[tokio::test]
    async fn load_returns_defaults_after_init() {
        let service = service().await;
        let settings = service.load().await.expect("读取应成功");

        assert_eq!(settings, AppSettings::default());
    }

    #[tokio::test]
    async fn update_persists_and_merges() {
        let service = service().await;
        let partial: PartialAppSettings = serde_json::from_value(serde_json::json!({
            "appearance": { "theme": "dark" }
        }))
        .expect("部分更新应可反序列化");

        let updated = service.update(&partial).await.expect("更新应成功");
        assert_eq!(updated.appearance.theme, ThemeMode::Dark);

        // 再次读取应持久化，且其他字段保持默认
        let reloaded = service.load().await.expect("读取应成功");
        assert_eq!(reloaded.appearance.theme, ThemeMode::Dark);
        assert_eq!(reloaded.editor.font_size, 13);
    }

    #[tokio::test]
    async fn update_rejects_invalid_range() {
        let service = service().await;
        let partial: PartialAppSettings = serde_json::from_value(serde_json::json!({
            "editor": { "fontSize": 200 }
        }))
        .expect("部分更新应可反序列化");

        let error = service.update(&partial).await.expect_err("非法值应被拒绝");
        assert_eq!(error.code, crate::errors::error_code::INVALID_ARGUMENT);

        // 拒绝后不应写入数据库
        let stored = settings_repository::load_map(service.db.pool())
            .await
            .expect("读取应成功");
        assert_eq!(stored.get(KEY_THEME).map(String::as_str), Some("system"));
    }

    #[tokio::test]
    async fn identical_update_is_noop() {
        let service = service().await;
        let partial: PartialAppSettings = serde_json::from_value(serde_json::json!({
            "appearance": { "theme": "system" }
        }))
        .expect("部分更新应可反序列化");

        let before = settings_repository::load_map(service.db.pool())
            .await
            .expect("读取应成功");
        let updated = service.update(&partial).await.expect("更新应成功");
        let after = settings_repository::load_map(service.db.pool())
            .await
            .expect("读取应成功");

        assert_eq!(updated.appearance.theme, ThemeMode::System);
        assert_eq!(before, after);
    }
}
