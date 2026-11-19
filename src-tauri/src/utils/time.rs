//! 时间与标识生成。
//!
//! 所有持久化时间统一使用 UTC Unix 毫秒（见 `docs/04` 2.5）。

use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 Unix 毫秒时间戳
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// 新的业务主键（UUID v4）
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 把 u64 时间戳转换为数据库使用的 i64
pub fn to_db_time(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_is_reasonable() {
        // 2020-01-01 之后、2100 年之前，避免明显的单位错误
        let now = now_ms();
        assert!(now > 1_577_836_800_000);
        assert!(now < 4_102_444_800_000);
    }

    #[test]
    fn new_id_is_uuid_shaped() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }
}
