//! 数据源连接配置模型（阶段 D 使用，阶段 C 先完成持久化）。
//!
//! 注意：本模型不包含密码，密码只保存在系统凭据服务中，
//! `credential_key` 仅作为凭据定位标识。

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceConfig {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub credential_key: Option<String>,
    pub default_database: Option<String>,
    pub options_json: Option<String>,
    pub remark: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 创建连接配置的输入（不含密码）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDataSource {
    pub name: String,
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub options_json: Option<String>,
    pub remark: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_expose_password_field() {
        let value = serde_json::to_value(DataSourceConfig {
            id: "id".into(),
            name: "本地".into(),
            source_type: "mysql".into(),
            host: "127.0.0.1".into(),
            port: 3306,
            username: "root".into(),
            credential_key: Some("datadock/id".into()),
            default_database: None,
            options_json: None,
            remark: None,
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
        })
        .expect("应可序列化");

        assert!(value.get("password").is_none());
        assert_eq!(value["credentialKey"], "datadock/id");
        assert_eq!(value["sourceType"], "mysql");
    }
}
