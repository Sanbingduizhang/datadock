//! SQL 收藏模型（阶段 E/H 接入界面时使用）。

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SqlFavorite {
    pub id: String,
    pub name: String,
    pub connection_id: Option<String>,
    pub database_name: Option<String>,
    pub sql_text: String,
    pub category: Option<String>,
    pub remark: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSqlFavorite {
    pub name: String,
    pub connection_id: Option<String>,
    pub database_name: Option<String>,
    pub sql_text: String,
    pub category: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSqlFavoriteRequest {
    pub favorite_id: String,
    #[serde(flatten)]
    pub favorite: NewSqlFavorite,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSqlFavoriteRequest {
    pub favorite_id: String,
}
