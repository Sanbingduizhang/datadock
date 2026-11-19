//! 系统凭据存储。
//!
//! 数据库密码只保存在操作系统的凭据服务中（macOS Keychain / Windows Credential Manager /
//! Linux Secret Service），SQLite 仅保存 `credential_key` 引用。

use keyring::Entry;

use crate::errors::AppError;

/// Keychain / Credential Manager 中的服务名，与 Tauri 应用标识保持一致
pub const SERVICE_NAME: &str = "com.datadock.app";

/// 凭据存储接口。阶段 D 的连接管理通过该接口读写密码。
pub trait CredentialStore: Send + Sync {
    fn set_secret(&self, key: &str, secret: &str) -> Result<(), AppError>;
    fn get_secret(&self, key: &str) -> Result<Option<String>, AppError>;
    fn delete_secret(&self, key: &str) -> Result<(), AppError>;
}

/// 基于操作系统原生凭据服务的实现。
pub struct KeyringCredentialStore {
    service: String,
}

impl KeyringCredentialStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> Result<Entry, AppError> {
        if key.trim().is_empty() {
            return Err(AppError::validation("凭据键不能为空"));
        }

        Entry::new(&self.service, key)
            .map_err(|error| AppError::credential(format!("访问系统凭据失败：{error}")))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn set_secret(&self, key: &str, secret: &str) -> Result<(), AppError> {
        self.entry(key)?
            .set_password(secret)
            .map_err(|error| AppError::credential(format!("保存密码到系统凭据失败：{error}")))
    }

    fn get_secret(&self, key: &str) -> Result<Option<String>, AppError> {
        match self.entry(key)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            // 凭据不存在与读取失败必须区分：不存在时允许上层提示用户重新输入
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::credential(format!("读取系统凭据失败：{error}"))),
        }
    }

    fn delete_secret(&self, key: &str) -> Result<(), AppError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::credential(format!("删除系统凭据失败：{error}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_key_is_rejected() {
        let store = KeyringCredentialStore::new(SERVICE_NAME);
        assert!(store.get_secret("  ").is_err());
    }

    /// 真实读写系统凭据。若当前环境无法访问凭据服务，本测试会失败而不是被忽略。
    #[test]
    fn keyring_round_trip() {
        let store = KeyringCredentialStore::new("com.datadock.app.test");
        let key = format!("datadock/round-trip-{}", uuid::Uuid::new_v4());

        store
            .set_secret(&key, "s3cret-value")
            .expect("应能写入凭据");
        let read = store.get_secret(&key).expect("应能读取凭据");
        assert_eq!(read.as_deref(), Some("s3cret-value"));

        store.delete_secret(&key).expect("应能删除凭据");
        let after_delete = store.get_secret(&key).expect("删除后读取不应报错");
        assert!(after_delete.is_none());
    }
}
