//! 统一错误码定义。
//!
//! 错误码用于程序判断，前端不得依赖 message 文本做逻辑分支。
//! 这里只维护当前已使用的错误码，后续模块按需补充（完整错误码表见
//! `docs/06-前后端接口设计.md` 第 20 章）。

/// 参数无效：请求字段缺失、格式错误或超出允许范围。
pub const INVALID_ARGUMENT: &str = "INVALID_ARGUMENT";

/// 本地存储错误：SQLite、文件系统等本地持久化失败。
pub const STORAGE_ERROR: &str = "STORAGE_ERROR";

/// 系统凭据访问失败：Keychain / Credential Manager 不可用或读写失败。
pub const CREDENTIAL_ACCESS_FAILED: &str = "CREDENTIAL_ACCESS_FAILED";

/// 连接配置不存在
pub const CONNECTION_NOT_FOUND: &str = "CONNECTION_NOT_FOUND";

/// 连接尚未建立
pub const CONNECTION_NOT_READY: &str = "CONNECTION_NOT_READY";

/// 连接失败（网络、认证、超时等）
pub const CONNECTION_FAILED: &str = "CONNECTION_FAILED";

/// 连接已经建立
pub const CONNECTION_ALREADY_CONNECTED: &str = "CONNECTION_ALREADY_CONNECTED";

/// 系统凭据不存在（需要用户重新输入密码）
pub const CREDENTIAL_NOT_FOUND: &str = "CREDENTIAL_NOT_FOUND";

/// 数据库或表不存在 / 无权限
pub const DATABASE_NOT_FOUND: &str = "DATABASE_NOT_FOUND";
pub const TABLE_NOT_FOUND: &str = "TABLE_NOT_FOUND";

/// SQL 内容为空
pub const SQL_EMPTY: &str = "SQL_EMPTY";

/// SQL 执行失败
pub const SQL_EXECUTION_FAILED: &str = "SQL_EXECUTION_FAILED";

/// 内部错误：未归类的程序异常，不向用户暴露堆栈细节。
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
