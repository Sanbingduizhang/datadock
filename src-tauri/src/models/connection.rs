//! 连接管理的数据模型与 IPC 契约（对应 `docs/06` 第 5 章）。

use serde::{Deserialize, Serialize};

pub const SOURCE_TYPE_MYSQL: &str = "mysql";
pub const SOURCE_TYPE_REDIS: &str = "redis";
pub const SOURCE_TYPE_POSTGRES: &str = "postgres";
pub const DEFAULT_POSTGRES_PORT: u16 = 5432;
pub const DEFAULT_REDIS_PORT: u16 = 6379;

/// 判断是否为已支持的数据源类型
pub fn is_supported_source_type(source_type: &str) -> bool {
    source_type.eq_ignore_ascii_case(SOURCE_TYPE_MYSQL)
        || source_type.eq_ignore_ascii_case(SOURCE_TYPE_REDIS)
        || source_type.eq_ignore_ascii_case(SOURCE_TYPE_POSTGRES)
}
pub const DEFAULT_MYSQL_PORT: u16 = 3306;
pub const DEFAULT_CHARSET: &str = "utf8mb4";
pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 8_000;
pub const MIN_CONNECT_TIMEOUT_MS: u64 = 1_000;
pub const MAX_CONNECT_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl ConnectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Error => "error",
        }
    }
}

/// SSH 隧道配置（运行时由 ConnectionOptions 解析而来，密码字段运行时填充）。
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: SshAuthType,
    /// 私钥认证时的私钥文件路径
    pub private_key_path: Option<String>,
    /// 密码认证时的密码（运行时从凭据填充）
    pub password: Option<String>,
}

/// 连接配置 + 运行时状态。绝不包含密码。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    /// 运行时是否已连接
    pub connected: bool,
    pub server_version: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// SSL / SSH 隧道等扩展选项（不含密码、私钥内容）
    pub options: ConnectionOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionRequest {
    pub name: String,
    #[serde(default = "default_source_type")]
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    pub connect_timeout_ms: Option<u64>,
    /// SSL / SSH 隧道等扩展选项
    #[serde(default)]
    pub options: Option<ConnectionOptions>,
}

fn default_source_type() -> String {
    SOURCE_TYPE_MYSQL.to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionResponse {
    pub connection_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionRequest {
    pub connection_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    pub connect_timeout_ms: Option<u64>,
    /// 不传或为空表示保留原密码
    pub new_password: Option<String>,
    /// SSL / SSH 隧道等扩展选项
    #[serde(default)]
    pub options: Option<ConnectionOptions>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSuccessResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionIdRequest {
    pub connection_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionRequest {
    #[serde(default = "default_source_type")]
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    pub connect_timeout_ms: Option<u64>,
    /// SSL / SSH 隧道等扩展选项
    #[serde(default)]
    pub options: Option<ConnectionOptions>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
    pub server_version: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectDatabaseResponse {
    pub connection_id: String,
    pub status: String,
    pub server_version: Option<String>,
    pub current_database: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectDatabaseResponse {
    pub connection_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRuntimeInfo {
    pub connection_id: String,
    pub status: String,
    pub current_database: Option<String>,
    pub connected_at: Option<i64>,
    pub last_error: Option<String>,
}

/// SSL 加密模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SslMode {
    /// 不加密
    Disabled,
    /// 尽量加密（PG 默认）
    #[default]
    Prefer,
    /// 强制加密但不校验证书
    Require,
    /// 加密并校验证书颁发机构
    VerifyCa,
    /// 加密并校验证书与主机名
    VerifyFull,
}

impl SslMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" | "disable" => Some(Self::Disabled),
            "prefer" | "preferred" => Some(Self::Prefer),
            "require" | "required" => Some(Self::Require),
            "verify-ca" | "verifyCA" => Some(Self::VerifyCa),
            "verify-full" | "verifyFull" => Some(Self::VerifyFull),
            _ => None,
        }
    }
}

/// SSH 隧道认证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthType {
    #[default]
    Password,
    PrivateKey,
}

impl SshAuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::PrivateKey => "privateKey",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "password" => Some(Self::Password),
            "privateKey" | "private-key" | "key" => Some(Self::PrivateKey),
            _ => None,
        }
    }
}

/// 连接扩展选项（SSL / SSH 隧道），序列化存入 options_json。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionOptions {
    pub ssl_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_mode: Option<SslMode>,
    pub ssh_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_auth_type: Option<SshAuthType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_private_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_local_port: Option<u16>,
}

/// 单条连接的导出条目（不含密码，密码在目标机器上重新输入）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionExportEntry {
    pub name: String,
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    pub connect_timeout_ms: Option<u64>,
    #[serde(default)]
    pub options: ConnectionOptions,
}

/// 连接配置导出文件结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionExportFile {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    pub connections: Vec<ConnectionExportEntry>,
}

/// 连接导入结果：成功与跳过（重名）的数量。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionImportResponse {
    pub imported: u32,
    pub skipped: u32,
}

/// 校验后的连接参数（不含密码），供适配器使用
#[derive(Debug, Clone)]
pub struct ConnectionTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub charset: Option<String>,
    pub connect_timeout_ms: u64,
    pub options: ConnectionOptions,
}

impl ConnectionTarget {
    pub fn from_create(request: &CreateConnectionRequest) -> Self {
        Self {
            host: request.host.trim().to_string(),
            port: request.port,
            username: request.username.trim().to_string(),
            default_database: normalize_optional(&request.default_database),
            charset: normalize_optional(&request.charset),
            connect_timeout_ms: normalize_timeout(request.connect_timeout_ms),
            options: request.options.clone().unwrap_or_default(),
        }
    }

    pub fn from_update(request: &UpdateConnectionRequest) -> Self {
        Self {
            host: request.host.trim().to_string(),
            port: request.port,
            username: request.username.trim().to_string(),
            default_database: normalize_optional(&request.default_database),
            charset: normalize_optional(&request.charset),
            connect_timeout_ms: normalize_timeout(request.connect_timeout_ms),
            options: request.options.clone().unwrap_or_default(),
        }
    }

    pub fn from_test(request: &TestConnectionRequest) -> Self {
        Self {
            host: request.host.trim().to_string(),
            port: request.port,
            username: request.username.trim().to_string(),
            default_database: normalize_optional(&request.default_database),
            charset: normalize_optional(&request.charset),
            connect_timeout_ms: normalize_timeout(request.connect_timeout_ms),
            options: request.options.clone().unwrap_or_default(),
        }
    }
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn normalize_timeout(value: Option<u64>) -> u64 {
    value
        .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS)
        .clamp(MIN_CONNECT_TIMEOUT_MS, MAX_CONNECT_TIMEOUT_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_empty_optional_fields() {
        let request = CreateConnectionRequest {
            name: " 本地 ".into(),
            source_type: "mysql".into(),
            host: " 127.0.0.1 ".into(),
            port: 3306,
            username: " root ".into(),
            password: "p".into(),
            default_database: Some("   ".into()),
            charset: None,
            connect_timeout_ms: Some(10),
            options: None,
        };

        let target = ConnectionTarget::from_create(&request);

        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.username, "root");
        assert!(target.default_database.is_none());
        assert_eq!(target.connect_timeout_ms, MIN_CONNECT_TIMEOUT_MS);
    }

    #[test]
    fn clamps_timeout() {
        assert_eq!(normalize_timeout(Some(999_999)), MAX_CONNECT_TIMEOUT_MS);
        assert_eq!(normalize_timeout(None), DEFAULT_CONNECT_TIMEOUT_MS);
    }
}
