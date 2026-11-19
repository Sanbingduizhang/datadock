//! 数据库对象模型（数据库 / 表 / 字段 / 索引）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableKind {
    Table,
    View,
}

impl TableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub table_kind: TableKind,
    pub engine: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTablesRequest {
    pub connection_id: String,
    pub database_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchemaRequest {
    pub connection_id: String,
    pub database_name: String,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    /// 完整类型，例如 `varchar(64)`、`decimal(12,2)`
    pub column_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    /// PRI / UNI / MUL / 空
    pub column_key: String,
    /// auto_increment 等附加属性
    pub extra: String,
    pub comment: String,
    pub position: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
    pub index_type: String,
    /// 复合索引按 SEQ_IN_INDEX 顺序保存
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDdlRequest {
    pub connection_id: String,
    pub database_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDdl {
    pub database_name: String,
    pub ddl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub ddl: String,
}

impl ColumnInfo {
    pub fn is_primary_key(&self) -> bool {
        self.column_key == "PRI"
    }

    pub fn is_auto_increment(&self) -> bool {
        self.extra.to_ascii_lowercase().contains("auto_increment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_kind_serializes_as_type_field() {
        let info = TableInfo {
            name: "users".into(),
            table_kind: TableKind::View,
            engine: None,
            comment: None,
        };

        let value = serde_json::to_value(&info).expect("应可序列化");
        assert_eq!(value["type"], "view");
        assert_eq!(value["name"], "users");
    }

    #[test]
    fn column_flags() {
        let column = ColumnInfo {
            name: "id".into(),
            column_type: "bigint unsigned".into(),
            nullable: false,
            default_value: None,
            column_key: "PRI".into(),
            extra: "auto_increment".into(),
            comment: "主键".into(),
            position: 1,
        };

        assert!(column.is_primary_key());
        assert!(column.is_auto_increment());
    }
}
