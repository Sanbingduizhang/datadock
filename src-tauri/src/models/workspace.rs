//! 工作区状态模型。
//!
//! 持久化结构以 `docs/04` 第 8 章（state_json 中的布局与标签）为基础，
//! 并保持与前端 `src/types/workspace.ts` 一致的标签形状，避免额外转换层。
//! 与 `docs/06` 15.1 的差异：标签增加了 `welcome` / `structure` 类型与 `context` 嵌套结构。

use serde::{Deserialize, Serialize};

/// 工作区状态的当前版本号，结构变更时递增
pub const CURRENT_WORKSPACE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceTabKind {
    Welcome,
    Query,
    Table,
    Structure,
    #[serde(rename = "redis-key")]
    RedisKey,
    #[serde(rename = "redis-db")]
    RedisDb,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TabContextState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    /// Redis 键名（redis-key 标签使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    /// Redis 库索引（redis-db / redis-key 标签使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redis_db_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTabState {
    pub id: String,
    #[serde(rename = "type")]
    pub tab_type: WorkspaceTabKind,
    pub title: String,
    #[serde(default)]
    pub context: TabContextState,
    /// SQL 查询标签的编辑内容；阶段 C 起随工作区一起保存
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayout {
    pub sidebar_width: u32,
    pub sidebar_visible: bool,
    pub result_panel_height: u32,
    pub result_panel_visible: bool,
    pub status_bar_visible: bool,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            sidebar_width: 250,
            sidebar_visible: true,
            result_panel_height: 220,
            result_panel_visible: true,
            status_bar_visible: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub version: u32,
    /// 工作区显示名（多工作区切换时展示）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_tab_id: Option<String>,
    #[serde(default)]
    pub tabs: Vec<WorkspaceTabState>,
    #[serde(default)]
    pub layout: WorkspaceLayout,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            version: CURRENT_WORKSPACE_VERSION,
            name: None,
            active_tab_id: None,
            tabs: Vec::new(),
            layout: WorkspaceLayout::default(),
        }
    }
}

/* ---------- IPC 请求/响应 ---------- */

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWorkspaceRequest {
    pub workspace_id: String,
    pub state: WorkspaceState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWorkspaceResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkspaceResponse {
    pub workspace_id: String,
    /// 首次启动时为空
    pub state: Option<WorkspaceState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_tab_types_round_trip_as_kebab() {
        let state = WorkspaceState {
            version: CURRENT_WORKSPACE_VERSION,
            tabs: vec![
                WorkspaceTabState {
                    id: "t1".to_string(),
                    tab_type: WorkspaceTabKind::RedisKey,
                    title: "key1".to_string(),
                    context: TabContextState {
                        redis_db_index: Some(3),
                        ..Default::default()
                    },
                    draft: None,
                },
                WorkspaceTabState {
                    id: "t2".to_string(),
                    tab_type: WorkspaceTabKind::RedisDb,
                    title: "db3".to_string(),
                    context: TabContextState {
                        redis_db_index: Some(3),
                        ..Default::default()
                    },
                    draft: None,
                },
            ],
            ..Default::default()
        };

        let json = serde_json::to_string(&state).expect("serialize");
        assert!(
            json.contains("\"redis-key\""),
            "should serialize RedisKey as redis-key: {json}"
        );
        assert!(
            json.contains("\"redis-db\""),
            "should serialize RedisDb as redis-db: {json}"
        );
        let restored: WorkspaceState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, state);
    }

    #[test]
    fn serializes_tab_type_as_type_field() {
        let tab = WorkspaceTabState {
            id: "tab-1".to_string(),
            tab_type: WorkspaceTabKind::Query,
            title: "SQL 查询 1".to_string(),
            context: TabContextState::default(),
            draft: None,
        };

        let value = serde_json::to_value(&tab).expect("标签应可序列化");

        assert_eq!(value["type"], "query");
        assert!(value.get("draft").is_none());
        assert!(value.get("context").is_some());
    }

    #[test]
    fn state_round_trip() {
        let state = WorkspaceState {
            version: CURRENT_WORKSPACE_VERSION,
            name: Some("测试工作区".to_string()),
            active_tab_id: Some("tab-2".to_string()),
            tabs: vec![WorkspaceTabState {
                id: "tab-2".to_string(),
                tab_type: WorkspaceTabKind::Table,
                title: "users".to_string(),
                context: TabContextState {
                    connection_id: Some("conn-1".to_string()),
                    connection_name: Some("本地".to_string()),
                    database_name: Some("tbdock".to_string()),
                    table_name: Some("users".to_string()),
                    key_name: None,
                    redis_db_index: None,
                },
                draft: None,
            }],
            layout: WorkspaceLayout::default(),
        };

        let json = serde_json::to_string(&state).expect("应可序列化");
        let restored: WorkspaceState = serde_json::from_str(&json).expect("应可反序列化");

        assert_eq!(restored, state);
    }

    #[test]
    fn layout_defaults_match_frontend() {
        let layout = WorkspaceLayout::default();

        assert_eq!(layout.sidebar_width, 250);
        assert_eq!(layout.result_panel_height, 220);
        assert!(layout.sidebar_visible);
        assert!(layout.result_panel_visible);
        assert!(layout.status_bar_visible);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    /// 持久化时间（Unix 毫秒）
    pub updated_at: i64,
    pub tab_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWorkspaceRequest {
    pub workspace_id: String,
}
