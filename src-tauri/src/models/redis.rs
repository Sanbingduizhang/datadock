//! Redis 键浏览模型。

use serde::{Deserialize, Serialize};

/// Redis 键类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisKeyType {
    String,
    Hash,
    List,
    Set,
    Zset,
    Stream,
    None,
}

impl RedisKeyType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Hash => "hash",
            Self::List => "list",
            Self::Set => "set",
            Self::Zset => "zset",
            Self::Stream => "stream",
            Self::None => "none",
        }
    }
}

/// Redis 键信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyInfo {
    /// 键名
    pub name: String,
    /// 键类型
    #[serde(rename = "type")]
    pub key_type: RedisKeyType,
    /// TTL 秒数（-2 键不存在，-1 无过期时间）
    pub ttl_seconds: i64,
}

/// SCAN 请求
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanRequest {
    pub connection_id: String,
    /// 目标库索引；缺省时用连接当前库，传入可在指定库上扫描（切换库浏览）
    pub database: Option<u32>,
    /// 匹配模式（SCAN MATCH），默认 "*"
    pub pattern: Option<String>,
    /// 每批扫描数量（SCAN COUNT），默认 100
    pub count: Option<u32>,
    /// 是否包含系统键（以 `_` 开头），默认 false
    #[serde(default)]
    pub include_system_keys: bool,
}

/// SCAN 响应
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanResponse {
    /// 键列表
    pub keys: Vec<RedisKeyInfo>,
    /// 游标（0 表示扫描完成）
    pub cursor: u64,
}

/// 键值请求
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyValueRequest {
    pub connection_id: String,
    /// 目标库索引；缺省时用连接当前库，传入可在指定库上读取键值
    pub database: Option<u32>,
    pub key_name: String,
    /// 列表/集合/有序集合的读取数量上限（防止大键卡顿），默认 100
    pub max_items: Option<u32>,
}

/// 键值响应（类型化的值内容）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyValueResponse {
    pub key_name: String,
    #[serde(rename = "type")]
    pub key_type: RedisKeyType,
    /// TTL 秒数
    pub ttl_seconds: i64,
    /// 值内容：根据类型展示不同形状
    pub value: RedisKeyValue,
}

/// 键值内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum RedisKeyValue {
    /// 简单字符串/数字
    String { value: Option<String> },
    /// Hash（字段-值对）
    Hash { fields: Vec<HashField> },
    /// List（索引-值对）
    List { items: Vec<ListItem> },
    /// Set（成员列表）
    Set { members: Vec<String> },
    /// ZSet（成员-分数对）
    Zset { entries: Vec<ZsetEntry> },
    /// Stream（简化的条目列表）
    Stream { entries: Vec<StreamEntry> },
    /// 键不存在或类型不支持
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashField {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListItem {
    pub index: u64,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZsetEntry {
    pub member: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEntry {
    pub id: String,
    pub fields: Vec<HashField>,
}

/* ---------- 写操作请求 ---------- */

/// 删除键
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisDeleteKeyRequest {
    pub connection_id: String,
    pub database: Option<u32>,
    pub key_name: String,
}

/// 重命名键
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisRenameKeyRequest {
    pub connection_id: String,
    pub database: Option<u32>,
    pub from: String,
    pub to: String,
}

/// 设置过期时间（秒；<=0 表示移除过期）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSetTtlRequest {
    pub connection_id: String,
    pub database: Option<u32>,
    pub key_name: String,
    pub ttl_seconds: i64,
}

/// 写入键值（编辑或新增）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSetKeyValueRequest {
    pub connection_id: String,
    pub database: Option<u32>,
    pub key_name: String,
    /// 键类型（新增时决定写入命令）
    pub key_type: RedisKeyType,
    /// 值内容
    pub value: RedisKeyValue,
    /// 过期时间（秒；None 不变，<=0 永不过期）
    pub ttl_seconds: Option<i64>,
}
