//! 应用设置模型。
//!
//! 存储契约以 `docs/04-数据库设计.md` 第 7 章为准（扁平 key + value_type），
//! 对外 IPC 结构以 `docs/06-前后端接口设计.md` 第 14 章为准（分组对象）。
//! 两者的字段映射集中在本文件的 `key_values` / `from_stored` 中，避免散落。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/* ---------- 配置键 ---------- */

pub const KEY_THEME: &str = "appearance.theme";
pub const KEY_FONT_SIZE: &str = "editor.font_size";
pub const KEY_TAB_SIZE: &str = "editor.tab_size";
pub const KEY_WORD_WRAP: &str = "editor.word_wrap";
pub const KEY_LINE_NUMBERS: &str = "editor.line_numbers";
pub const KEY_PAGE_SIZE: &str = "data.default_page_size";
pub const KEY_MAX_ROWS: &str = "data.max_result_rows";
pub const KEY_HISTORY_ENABLED: &str = "query.history_enabled";
pub const KEY_HISTORY_LIMIT: &str = "query.history_limit";
pub const KEY_QUERY_TIMEOUT: &str = "query.default_timeout";
pub const KEY_RESTORE_WORKSPACE: &str = "general.restore_workspace";
pub const KEY_REMEMBER_SIZE: &str = "window.remember_size";
pub const KEY_REMEMBER_POSITION: &str = "window.remember_position";

/// 首次初始化写入的默认值。使用 `INSERT OR IGNORE`，不会覆盖用户已修改的设置。
pub const DEFAULT_SETTINGS: &[(&str, &str, &str)] = &[
    (KEY_THEME, "system", "string"),
    (KEY_FONT_SIZE, "13", "number"),
    (KEY_TAB_SIZE, "2", "number"),
    (KEY_WORD_WRAP, "off", "string"),
    (KEY_LINE_NUMBERS, "true", "boolean"),
    (KEY_PAGE_SIZE, "100", "number"),
    (KEY_MAX_ROWS, "10000", "number"),
    (KEY_HISTORY_ENABLED, "true", "boolean"),
    (KEY_HISTORY_LIMIT, "1000", "number"),
    (KEY_QUERY_TIMEOUT, "30", "number"),
    (KEY_RESTORE_WORKSPACE, "true", "boolean"),
    (KEY_REMEMBER_SIZE, "true", "boolean"),
    (KEY_REMEMBER_POSITION, "true", "boolean"),
];

/* ---------- 数值范围 ---------- */

pub const FONT_SIZE_RANGE: (u16, u16) = (10, 32);
pub const TAB_SIZE_RANGE: (u8, u8) = (1, 8);
pub const PAGE_SIZE_RANGE: (u16, u16) = (1, 1000);
pub const MAX_ROWS_RANGE: (u32, u32) = (100, 1_000_000);

/* ---------- 枚举 ---------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
    /// 玻璃拟态（半透明磨砂面板）
    Glass,
    /// 拟物风格（带纹理与压痕的高阶表面）
    Skeuomorph,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
            Self::Glass => "glass",
            Self::Skeuomorph => "skeuomorph",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            "glass" => Some(Self::Glass),
            "skeuomorph" => Some(Self::Skeuomorph),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WordWrap {
    On,
    Off,
}

impl WordWrap {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/* ---------- 完整设置 ---------- */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: ThemeMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub font_size: u16,
    pub tab_size: u8,
    pub word_wrap: WordWrap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySettings {
    pub default_max_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSettings {
    pub default_page_size: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySettings {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub restore_workspace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub appearance: AppearanceSettings,
    pub editor: EditorSettings,
    pub query: QuerySettings,
    pub data: DataSettings,
    pub history: HistorySettings,
    pub general: GeneralSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings {
                theme: ThemeMode::System,
            },
            editor: EditorSettings {
                font_size: 13,
                tab_size: 2,
                word_wrap: WordWrap::Off,
            },
            query: QuerySettings {
                default_max_rows: 10_000,
            },
            data: DataSettings {
                default_page_size: 100,
            },
            history: HistorySettings { enabled: true },
            general: GeneralSettings {
                restore_workspace: true,
            },
        }
    }
}

impl AppSettings {
    /// 从数据库键值还原设置，缺失或非法值回退到默认值。
    pub fn from_stored(stored: &HashMap<String, String>) -> Self {
        let defaults = Self::default();

        Self {
            appearance: AppearanceSettings {
                theme: text(stored, KEY_THEME)
                    .and_then(|value| ThemeMode::parse(&value))
                    .unwrap_or(defaults.appearance.theme),
            },
            editor: EditorSettings {
                font_size: number(stored, KEY_FONT_SIZE)
                    .filter(|value| in_range_u16(*value, FONT_SIZE_RANGE))
                    .unwrap_or(defaults.editor.font_size),
                tab_size: number(stored, KEY_TAB_SIZE)
                    .filter(|value| in_range_u8(*value, TAB_SIZE_RANGE))
                    .unwrap_or(defaults.editor.tab_size),
                word_wrap: text(stored, KEY_WORD_WRAP)
                    .and_then(|value| WordWrap::parse(&value))
                    .unwrap_or(defaults.editor.word_wrap),
            },
            query: QuerySettings {
                default_max_rows: number(stored, KEY_MAX_ROWS)
                    .filter(|value| in_range_u32(*value, MAX_ROWS_RANGE))
                    .unwrap_or(defaults.query.default_max_rows),
            },
            data: DataSettings {
                default_page_size: number(stored, KEY_PAGE_SIZE)
                    .filter(|value| in_range_u16(*value, PAGE_SIZE_RANGE))
                    .unwrap_or(defaults.data.default_page_size),
            },
            history: HistorySettings {
                enabled: boolean(stored, KEY_HISTORY_ENABLED).unwrap_or(defaults.history.enabled),
            },
            general: GeneralSettings {
                restore_workspace: boolean(stored, KEY_RESTORE_WORKSPACE)
                    .unwrap_or(defaults.general.restore_workspace),
            },
        }
    }

    /// 序列化为 (key, value, value_type) 三元组，用于批量写入。
    pub fn key_values(&self) -> Vec<(&'static str, String, &'static str)> {
        vec![
            (
                KEY_THEME,
                self.appearance.theme.as_str().to_string(),
                "string",
            ),
            (KEY_FONT_SIZE, self.editor.font_size.to_string(), "number"),
            (KEY_TAB_SIZE, self.editor.tab_size.to_string(), "number"),
            (
                KEY_WORD_WRAP,
                self.editor.word_wrap.as_str().to_string(),
                "string",
            ),
            (
                KEY_PAGE_SIZE,
                self.data.default_page_size.to_string(),
                "number",
            ),
            (
                KEY_MAX_ROWS,
                self.query.default_max_rows.to_string(),
                "number",
            ),
            (
                KEY_HISTORY_ENABLED,
                self.history.enabled.to_string(),
                "boolean",
            ),
            (
                KEY_RESTORE_WORKSPACE,
                self.general.restore_workspace.to_string(),
                "boolean",
            ),
        ]
    }

    /// 校验数值范围，非法时返回 INVALID_ARGUMENT。
    pub fn validate(&self) -> Result<(), AppError> {
        if !in_range_u16(self.editor.font_size, FONT_SIZE_RANGE) {
            return Err(AppError::validation(format!(
                "编辑器字号需要在 {}~{} 之间",
                FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1
            )));
        }

        if !in_range_u8(self.editor.tab_size, TAB_SIZE_RANGE) {
            return Err(AppError::validation(format!(
                "缩进宽度需要在 {}~{} 之间",
                TAB_SIZE_RANGE.0, TAB_SIZE_RANGE.1
            )));
        }

        if !in_range_u16(self.data.default_page_size, PAGE_SIZE_RANGE) {
            return Err(AppError::validation(format!(
                "分页条数需要在 {}~{} 之间",
                PAGE_SIZE_RANGE.0, PAGE_SIZE_RANGE.1
            )));
        }

        if !in_range_u32(self.query.default_max_rows, MAX_ROWS_RANGE) {
            return Err(AppError::validation(format!(
                "结果行数上限需要在 {}~{} 之间",
                MAX_ROWS_RANGE.0, MAX_ROWS_RANGE.1
            )));
        }

        Ok(())
    }

    /// 合并部分更新（只覆盖显式传入的字段）。
    pub fn merged(&self, partial: &PartialAppSettings) -> Self {
        let mut next = self.clone();

        if let Some(theme) = partial.appearance.theme {
            next.appearance.theme = theme;
        }
        if let Some(font_size) = partial.editor.font_size {
            next.editor.font_size = font_size;
        }
        if let Some(tab_size) = partial.editor.tab_size {
            next.editor.tab_size = tab_size;
        }
        if let Some(word_wrap) = partial.editor.word_wrap {
            next.editor.word_wrap = word_wrap;
        }
        if let Some(max_rows) = partial.query.default_max_rows {
            next.query.default_max_rows = max_rows;
        }
        if let Some(page_size) = partial.data.default_page_size {
            next.data.default_page_size = page_size;
        }
        if let Some(enabled) = partial.history.enabled {
            next.history.enabled = enabled;
        }
        if let Some(restore) = partial.general.restore_workspace {
            next.general.restore_workspace = restore;
        }

        next
    }
}

/* ---------- 部分更新 ---------- */

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialAppearance {
    pub theme: Option<ThemeMode>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialEditor {
    pub font_size: Option<u16>,
    pub tab_size: Option<u8>,
    pub word_wrap: Option<WordWrap>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialQuery {
    pub default_max_rows: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialData {
    pub default_page_size: Option<u16>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialHistory {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialGeneral {
    pub restore_workspace: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialAppSettings {
    pub appearance: PartialAppearance,
    pub editor: PartialEditor,
    pub query: PartialQuery,
    pub data: PartialData,
    pub history: PartialHistory,
    pub general: PartialGeneral,
}

/* ---------- IPC 请求/响应 ---------- */

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub settings: PartialAppSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsResponse {
    pub success: bool,
}

/* ---------- 解析辅助 ---------- */

fn text(stored: &HashMap<String, String>, key: &str) -> Option<String> {
    stored.get(key).cloned()
}

fn number<T: std::str::FromStr>(stored: &HashMap<String, String>, key: &str) -> Option<T> {
    stored.get(key).and_then(|value| value.parse::<T>().ok())
}

fn boolean(stored: &HashMap<String, String>, key: &str) -> Option<bool> {
    stored.get(key).and_then(|value| match value.as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    })
}

fn in_range_u16(value: u16, range: (u16, u16)) -> bool {
    value >= range.0 && value <= range.1
}

fn in_range_u8(value: u8, range: (u8, u8)) -> bool {
    value >= range.0 && value <= range.1
}

fn in_range_u32(value: u32, range: (u32, u32)) -> bool {
    value >= range.0 && value <= range.1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn defaults_match_design_document() {
        let settings = AppSettings::default();

        assert_eq!(settings.appearance.theme, ThemeMode::System);
        assert_eq!(settings.editor.font_size, 13);
        assert_eq!(settings.editor.tab_size, 2);
        assert_eq!(settings.data.default_page_size, 100);
        assert_eq!(settings.query.default_max_rows, 10_000);
        assert!(settings.history.enabled);
    }

    #[test]
    fn reads_stored_values() {
        let stored = map(&[
            (KEY_THEME, "dark"),
            (KEY_FONT_SIZE, "15"),
            (KEY_WORD_WRAP, "on"),
            (KEY_PAGE_SIZE, "200"),
            (KEY_HISTORY_ENABLED, "false"),
        ]);

        let settings = AppSettings::from_stored(&stored);

        assert_eq!(settings.appearance.theme, ThemeMode::Dark);
        assert_eq!(settings.editor.font_size, 15);
        assert_eq!(settings.editor.word_wrap, WordWrap::On);
        assert_eq!(settings.data.default_page_size, 200);
        assert!(!settings.history.enabled);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let stored = map(&[
            (KEY_THEME, "rainbow"),
            (KEY_FONT_SIZE, "999"),
            (KEY_PAGE_SIZE, "abc"),
            (KEY_HISTORY_ENABLED, "maybe"),
        ]);

        let settings = AppSettings::from_stored(&stored);
        let defaults = AppSettings::default();

        assert_eq!(settings.appearance.theme, defaults.appearance.theme);
        assert_eq!(settings.editor.font_size, defaults.editor.font_size);
        assert_eq!(
            settings.data.default_page_size,
            defaults.data.default_page_size
        );
        assert_eq!(settings.history.enabled, defaults.history.enabled);
    }

    #[test]
    fn key_values_round_trip() {
        let settings = AppSettings::default();
        let stored: HashMap<String, String> = settings
            .key_values()
            .into_iter()
            .map(|(key, value, _)| (key.to_string(), value))
            .collect();

        assert_eq!(AppSettings::from_stored(&stored), settings);
    }

    #[test]
    fn merged_only_overrides_provided_fields() {
        let base = AppSettings::default();
        let partial: PartialAppSettings = serde_json::from_value(serde_json::json!({
            "appearance": { "theme": "dark" },
            "editor": { "fontSize": 18 }
        }))
        .expect("部分更新应可反序列化");

        let next = base.merged(&partial);

        assert_eq!(next.appearance.theme, ThemeMode::Dark);
        assert_eq!(next.editor.font_size, 18);
        // 未传字段保持不变
        assert_eq!(next.editor.tab_size, base.editor.tab_size);
        assert_eq!(next.data.default_page_size, base.data.default_page_size);
    }

    #[test]
    fn validate_rejects_out_of_range() {
        let mut settings = AppSettings::default();
        settings.editor.font_size = 99;

        let error = settings.validate().expect_err("超范围字号应被拒绝");
        assert_eq!(error.code, crate::errors::error_code::INVALID_ARGUMENT);
    }

    #[test]
    fn validate_accepts_defaults() {
        assert!(AppSettings::default().validate().is_ok());
    }
}
