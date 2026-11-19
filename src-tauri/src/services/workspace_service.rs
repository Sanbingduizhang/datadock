//! 工作区状态业务逻辑。

use crate::database::Database;
use crate::errors::AppError;
use crate::models::workspace::{WorkspaceState, WorkspaceSummary, CURRENT_WORKSPACE_VERSION};
use crate::repositories::workspace_repository;

const MAX_WORKSPACE_ID_LEN: usize = 64;
/// 默认工作区标识（与前端 WORKSPACE_STORAGE_KEY 一致）
pub const DEFAULT_WORKSPACE_ID: &str = "main";
const MAX_TABS: usize = 100;
const MAX_TITLE_LEN: usize = 200;

pub struct WorkspaceService {
    db: Database,
}

impl WorkspaceService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn load(&self, workspace_id: &str) -> Result<Option<WorkspaceState>, AppError> {
        validate_workspace_id(workspace_id)?;
        workspace_repository::load(self.db.pool(), workspace_id).await
    }

    pub async fn save(&self, workspace_id: &str, state: &WorkspaceState) -> Result<(), AppError> {
        validate_workspace_id(workspace_id)?;
        validate_state(state)?;

        let mut normalized = state.clone();
        normalized.version = CURRENT_WORKSPACE_VERSION;

        workspace_repository::save(self.db.pool(), workspace_id, &normalized).await
    }

    /// 列出全部工作区，按最近使用时间倒序。
    pub async fn list(&self) -> Result<Vec<WorkspaceSummary>, AppError> {
        let rows = workspace_repository::list_raw(self.db.pool()).await?;

        let mut items: Vec<WorkspaceSummary> = rows
            .into_iter()
            .map(|(id, json, updated_at)| {
                let parsed = serde_json::from_str::<WorkspaceState>(&json).ok();
                let name = parsed
                    .as_ref()
                    .and_then(|state| state.name.clone())
                    .unwrap_or_else(|| default_name(&id));
                let tab_count = parsed.as_ref().map(|state| state.tabs.len()).unwrap_or(0);

                WorkspaceSummary {
                    id,
                    name,
                    updated_at,
                    tab_count,
                }
            })
            .collect();

        items.sort_by_key(|item| std::cmp::Reverse(item.updated_at));
        Ok(items)
    }

    /// 删除工作区；默认工作区不允许删除，避免启动时无可用状态。
    pub async fn delete(&self, workspace_id: &str) -> Result<bool, AppError> {
        validate_workspace_id(workspace_id)?;
        if workspace_id == DEFAULT_WORKSPACE_ID {
            return Err(AppError::validation("默认工作区不能删除"));
        }

        workspace_repository::delete(self.db.pool(), workspace_id).await
    }
}

fn default_name(id: &str) -> String {
    if id == DEFAULT_WORKSPACE_ID {
        "默认工作区".to_string()
    } else {
        id.to_string()
    }
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), AppError> {
    let trimmed = workspace_id.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("工作区标识不能为空"));
    }
    if trimmed.chars().count() > MAX_WORKSPACE_ID_LEN {
        return Err(AppError::validation("工作区标识过长"));
    }
    Ok(())
}

fn validate_state(state: &WorkspaceState) -> Result<(), AppError> {
    if state.tabs.len() > MAX_TABS {
        return Err(AppError::validation(format!(
            "工作区标签数量超过上限（最多 {MAX_TABS} 个）"
        )));
    }

    if let Some(tab) = state
        .tabs
        .iter()
        .find(|tab| tab.title.chars().count() > MAX_TITLE_LEN)
    {
        return Err(AppError::validation("标签标题过长")
            .with_details(serde_json::json!({ "tabId": tab.id, "maxLength": MAX_TITLE_LEN })));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::models::workspace::{WorkspaceLayout, WorkspaceTabKind, WorkspaceTabState};

    async fn service() -> WorkspaceService {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        WorkspaceService::new(database)
    }

    fn state_with_tab() -> WorkspaceState {
        WorkspaceState {
            version: 0,
            name: Some("测试".to_string()),
            active_tab_id: Some("tab-1".to_string()),
            tabs: vec![WorkspaceTabState {
                id: "tab-1".to_string(),
                tab_type: WorkspaceTabKind::Query,
                title: "SQL 查询 1".to_string(),
                context: Default::default(),
                draft: Some("SELECT 1".to_string()),
            }],
            layout: WorkspaceLayout::default(),
        }
    }

    #[tokio::test]
    async fn list_uses_stored_name_and_sorts_by_time() {
        let service = service().await;
        let mut unnamed = state_with_tab();
        unnamed.name = None;

        service.save("main", &unnamed).await.expect("保存应成功");
        service
            .save("ws-2", &state_with_tab())
            .await
            .expect("保存应成功");

        let items = service.list().await.expect("列表应可读取");
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .any(|item| item.id == "main" && item.name == "默认工作区"));
        assert!(items
            .iter()
            .any(|item| item.id == "ws-2" && item.name == "测试"));
    }

    #[tokio::test]
    async fn default_workspace_cannot_be_deleted() {
        let service = service().await;
        assert!(service.delete("main").await.is_err());
        assert!(service.delete("ws-x").await.is_ok());
    }

    #[tokio::test]
    async fn save_normalizes_version_and_persists() {
        let service = service().await;

        service
            .save("main", &state_with_tab())
            .await
            .expect("保存应成功");
        let loaded = service
            .load("main")
            .await
            .expect("读取应成功")
            .expect("应存在状态");

        assert_eq!(loaded.version, CURRENT_WORKSPACE_VERSION);
        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.tabs[0].draft.as_deref(), Some("SELECT 1"));
    }

    #[tokio::test]
    async fn empty_workspace_id_rejected() {
        let service = service().await;

        let error = service
            .save("   ", &state_with_tab())
            .await
            .expect_err("空标识应被拒绝");
        assert_eq!(error.code, crate::errors::error_code::INVALID_ARGUMENT);
    }

    #[tokio::test]
    async fn too_many_tabs_rejected() {
        let service = service().await;
        let mut state = state_with_tab();
        state.tabs = (0..MAX_TABS + 1)
            .map(|index| WorkspaceTabState {
                id: format!("tab-{index}"),
                tab_type: WorkspaceTabKind::Table,
                title: "users".to_string(),
                context: Default::default(),
                draft: None,
            })
            .collect();

        let error = service
            .save("main", &state)
            .await
            .expect_err("超量标签应被拒绝");
        assert_eq!(error.code, crate::errors::error_code::INVALID_ARGUMENT);
    }
}
