//! 工作区状态持久化（`workspace_state` 表，按 state_key 唯一）。

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::models::workspace::WorkspaceState;
use crate::utils::time::{new_id, now_ms, to_db_time};

const UPSERT: &str = r#"
INSERT INTO workspace_state (id, state_key, state_json, updated_at)
VALUES (?, ?, ?, ?)
ON CONFLICT(state_key) DO UPDATE SET
    state_json = excluded.state_json,
    updated_at = excluded.updated_at
"#;

/// 读取工作区状态。JSON 损坏时返回 None 并记录警告，不阻断应用启动。
pub async fn load(pool: &SqlitePool, state_key: &str) -> Result<Option<WorkspaceState>, AppError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT state_json FROM workspace_state WHERE state_key = ?")
            .bind(state_key)
            .fetch_optional(pool)
            .await
            .map_err(|error| AppError::storage(format!("读取工作区状态失败：{error}")))?;

    let Some((json,)) = row else {
        return Ok(None);
    };

    match serde_json::from_str::<WorkspaceState>(&json) {
        Ok(state) => Ok(Some(state)),
        Err(error) => {
            tracing::warn!(target: "datadock::storage", key = state_key, %error, "工作区状态解析失败，按空状态处理");
            Ok(None)
        }
    }
}

/// 保存工作区状态（同 state_key 覆盖）。
pub async fn save(
    pool: &SqlitePool,
    state_key: &str,
    state: &WorkspaceState,
) -> Result<(), AppError> {
    let json = serde_json::to_string(state)
        .map_err(|error| AppError::internal(format!("工作区状态序列化失败：{error}")))?;

    sqlx::query(UPSERT)
        .bind(new_id())
        .bind(state_key)
        .bind(json)
        .bind(to_db_time(now_ms()))
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("保存工作区状态失败：{error}")))?;

    Ok(())
}

/// 列出全部工作区（原始 JSON 由 Service 解析名称与标签数）
pub async fn list_raw(pool: &SqlitePool) -> Result<Vec<(String, String, i64)>, AppError> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT state_key, state_json, updated_at FROM workspace_state")
            .fetch_all(pool)
            .await
            .map_err(|error| AppError::storage(format!("读取工作区列表失败：{error}")))?;

    Ok(rows)
}

pub async fn delete(pool: &SqlitePool, state_key: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM workspace_state WHERE state_key = ?")
        .bind(state_key)
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("删除工作区失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::models::workspace::{
        TabContextState, WorkspaceLayout, WorkspaceTabKind, WorkspaceTabState,
    };

    async fn setup() -> Database {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        database
    }

    fn sample_state() -> WorkspaceState {
        WorkspaceState {
            version: 1,
            name: Some("默认工作区".to_string()),
            active_tab_id: Some("tab-1".to_string()),
            tabs: vec![WorkspaceTabState {
                id: "tab-1".to_string(),
                tab_type: WorkspaceTabKind::Query,
                title: "SQL 查询 1".to_string(),
                context: TabContextState::default(),
                draft: Some("SELECT 1;".to_string()),
            }],
            layout: WorkspaceLayout {
                sidebar_width: 300,
                sidebar_visible: false,
                result_panel_height: 180,
                result_panel_visible: true,
                status_bar_visible: false,
            },
        }
    }

    #[tokio::test]
    async fn load_returns_none_when_absent() {
        let database = setup().await;

        let state = load(database.pool(), "main").await.expect("读取不应报错");

        assert!(state.is_none());
    }

    #[tokio::test]
    async fn save_then_load_round_trip() {
        let database = setup().await;
        let state = sample_state();

        save(database.pool(), "main", &state)
            .await
            .expect("保存应成功");
        let loaded = load(database.pool(), "main")
            .await
            .expect("读取应成功")
            .expect("应存在状态");

        assert_eq!(loaded, state);
    }

    #[tokio::test]
    async fn save_overwrites_same_key_without_duplication() {
        let database = setup().await;
        let mut state = sample_state();

        save(database.pool(), "main", &state)
            .await
            .expect("保存应成功");
        state.layout.sidebar_width = 380;
        save(database.pool(), "main", &state)
            .await
            .expect("保存应成功");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspace_state")
            .fetch_one(database.pool())
            .await
            .expect("应能统计");
        assert_eq!(count.0, 1);

        let loaded = load(database.pool(), "main")
            .await
            .expect("读取应成功")
            .expect("应存在状态");
        assert_eq!(loaded.layout.sidebar_width, 380);
    }

    #[tokio::test]
    async fn broken_json_yields_none() {
        let database = setup().await;
        sqlx::query(
            "INSERT INTO workspace_state (id, state_key, state_json, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind("id-1")
        .bind("main")
        .bind("{not-json}")
        .bind(0_i64)
        .execute(database.pool())
        .await
        .expect("插入应成功");

        let state = load(database.pool(), "main").await.expect("读取不应报错");

        assert!(state.is_none());
    }
}
