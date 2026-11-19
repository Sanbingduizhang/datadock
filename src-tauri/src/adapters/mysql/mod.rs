//! MySQL 数据源适配器。

pub mod connection;
pub mod dump;
pub mod explorer;
pub mod export;
pub mod query;
pub mod schema;
pub mod table;

use crate::errors::AppError;

/// 安全引用 MySQL 标识符（库名/表名）。
///
/// 标识符无法使用绑定参数，因此必须严格处理：拒绝空值与 NUL，转义反引号。
pub fn quote_identifier(name: &str) -> Result<String, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::validation("数据库对象名称不能为空"));
    }
    if name.contains('\0') {
        return Err(AppError::validation("数据库对象名称包含非法字符"));
    }

    Ok(format!("`{}`", name.replace('`', "``")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_identifier() {
        assert_eq!(quote_identifier("users").expect("应成功"), "`users`");
    }

    #[test]
    fn escapes_backtick() {
        assert_eq!(quote_identifier("we`ird").expect("应成功"), "`we``ird`");
    }

    #[test]
    fn rejects_empty_and_nul() {
        assert!(quote_identifier("   ").is_err());
        assert!(quote_identifier("a\0b").is_err());
    }
}
