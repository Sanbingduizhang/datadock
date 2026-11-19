//! 查询历史持久化。

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::models::history::{
    NewQueryHistory, QueryHistoryEntry, QueryHistoryOutcome, QueryHistoryStatus,
};
use crate::utils::time::{new_id, now_ms, to_db_time};

const SELECT_COLUMNS: &str = "id, connection_id, source_type, database_name, sql_text, status, \
     returned_rows, affected_rows, elapsed_ms, error_message, started_at, finished_at";

/// 开始执行时写入一条 running 记录，返回历史 ID。
pub async fn insert(pool: &SqlitePool, input: &NewQueryHistory) -> Result<String, AppError> {
    let id = new_id();
    let now = to_db_time(now_ms());

    sqlx::query(
        "INSERT INTO query_history \
         (id, connection_id, source_type, database_name, sql_text, status, \
          returned_rows, affected_rows, elapsed_ms, error_message, started_at, finished_at) \
         VALUES (?, ?, ?, ?, ?, ?, 0, NULL, 0, NULL, ?, NULL)",
    )
    .bind(&id)
    .bind(&input.connection_id)
    .bind(&input.source_type)
    .bind(&input.database_name)
    .bind(&input.sql_text)
    .bind(QueryHistoryStatus::Running.as_str())
    .bind(now)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("写入查询历史失败：{error}")))?;

    Ok(id)
}

/// 执行结束后更新状态与统计信息。
pub async fn finish(
    pool: &SqlitePool,
    id: &str,
    outcome: &QueryHistoryOutcome,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE query_history SET \
         status = ?, returned_rows = ?, affected_rows = ?, elapsed_ms = ?, \
         error_message = ?, finished_at = ? \
         WHERE id = ?",
    )
    .bind(outcome.status.as_str())
    .bind(outcome.returned_rows)
    .bind(outcome.affected_rows)
    .bind(outcome.elapsed_ms)
    .bind(&outcome.error_message)
    .bind(outcome.finished_at.unwrap_or(to_db_time(now_ms())))
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("更新查询历史失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

/// 按时间倒序分页读取历史，可按连接与关键字过滤。
pub async fn list(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
    connection_id: Option<&str>,
    keyword: Option<&str>,
) -> Result<Vec<QueryHistoryEntry>, AppError> {
    let keyword = keyword
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));

    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM query_history \
         WHERE (?1 IS NULL OR connection_id = ?1) \
           AND (?2 IS NULL OR sql_text LIKE ?2) \
         ORDER BY started_at DESC LIMIT ?3 OFFSET ?4"
    );

    sqlx::query_as::<_, QueryHistoryEntry>(&sql)
        .bind(connection_id)
        .bind(keyword.as_deref())
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取查询历史失败：{error}")))
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<QueryHistoryEntry>, AppError> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM query_history WHERE id = ?");

    sqlx::query_as::<_, QueryHistoryEntry>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|error| AppError::storage(format!("读取查询历史失败：{error}")))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM query_history WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("删除查询历史失败：{error}")))?;

    Ok(result.rows_affected() > 0)
}

pub async fn clear(pool: &SqlitePool) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM query_history")
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("清空查询历史失败：{error}")))?;

    Ok(result.rows_affected())
}

/// 清空指定连接的历史；`connection_id` 为空时清空全部。
pub async fn clear_for_connection(
    pool: &SqlitePool,
    connection_id: Option<&str>,
) -> Result<u64, AppError> {
    let Some(connection_id) = connection_id else {
        return clear(pool).await;
    };

    let result = sqlx::query("DELETE FROM query_history WHERE connection_id = ?")
        .bind(connection_id)
        .execute(pool)
        .await
        .map_err(|error| AppError::storage(format!("清空查询历史失败：{error}")))?;

    Ok(result.rows_affected())
}

pub async fn count(pool: &SqlitePool) -> Result<i64, AppError> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM query_history")
        .fetch_one(pool)
        .await
        .map_err(|error| AppError::storage(format!("统计查询历史失败：{error}")))?;

    Ok(count)
}

/// 保留最近 `keep` 条记录，删除更早的历史（对应 docs/04 5.6）。
pub async fn prune(pool: &SqlitePool, keep: u32) -> Result<u64, AppError> {
    let result = sqlx::query(
        "DELETE FROM query_history WHERE id NOT IN (\
            SELECT id FROM query_history ORDER BY started_at DESC LIMIT ?\
         )",
    )
    .bind(i64::from(keep))
    .execute(pool)
    .await
    .map_err(|error| AppError::storage(format!("清理查询历史失败：{error}")))?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    fn new_history(sql: &str, connection_id: Option<&str>) -> NewQueryHistory {
        NewQueryHistory {
            connection_id: connection_id.map(str::to_string),
            source_type: "mysql".to_string(),
            database_name: Some("tbdock".to_string()),
            sql_text: sql.to_string(),
        }
    }

    async fn setup() -> Database {
        let database = Database::in_memory().await.expect("内存数据库应可创建");
        database.migrate().await.expect("迁移应成功");
        database
    }

    #[tokio::test]
    async fn insert_starts_as_running() {
        let database = setup().await;
        let id = insert(database.pool(), &new_history("SELECT 1", Some("conn-1")))
            .await
            .expect("插入应成功");

        let entry = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");

        assert_eq!(entry.status, "running");
        assert_eq!(entry.returned_rows, 0);
        assert!(entry.finished_at.is_none());
    }

    #[tokio::test]
    async fn finish_updates_statistics() {
        let database = setup().await;
        let id = insert(database.pool(), &new_history("SELECT 1", None))
            .await
            .expect("插入应成功");

        let outcome = QueryHistoryOutcome {
            status: QueryHistoryStatus::Success,
            returned_rows: 3,
            affected_rows: None,
            elapsed_ms: 12,
            error_message: None,
            finished_at: Some(1_700_000_000_000),
        };
        assert!(finish(database.pool(), &id, &outcome)
            .await
            .expect("更新应成功"));

        let entry = get(database.pool(), &id)
            .await
            .expect("读取应成功")
            .expect("应存在记录");
        assert_eq!(entry.status, "success");
        assert_eq!(entry.returned_rows, 3);
        assert_eq!(entry.elapsed_ms, 12);
        assert_eq!(entry.finished_at, Some(1_700_000_000_000));
    }

    #[tokio::test]
    async fn list_orders_by_started_at_desc_and_filters() {
        let database = setup().await;
        let first = insert(database.pool(), &new_history("SELECT 1", Some("conn-1")))
            .await
            .expect("插入应成功");
        let second = insert(database.pool(), &new_history("SELECT 2", Some("conn-2")))
            .await
            .expect("插入应成功");

        sqlx::query("UPDATE query_history SET started_at = 100 WHERE id = ?")
            .bind(&first)
            .execute(database.pool())
            .await
            .expect("更新时间应成功");
        sqlx::query("UPDATE query_history SET started_at = 200 WHERE id = ?")
            .bind(&second)
            .execute(database.pool())
            .await
            .expect("更新时间应成功");

        let all = list(database.pool(), 10, 0, None, None)
            .await
            .expect("列表应可读取");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, second);

        let filtered = list(database.pool(), 10, 0, Some("conn-1"), None)
            .await
            .expect("筛选应可读取");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, first);

        let searched = list(database.pool(), 10, 0, None, Some("SELECT 2"))
            .await
            .expect("搜索应可读取");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].id, second);
    }

    #[tokio::test]
    async fn prune_keeps_recent_records() {
        let database = setup().await;
        for index in 0..5 {
            let id = insert(
                database.pool(),
                &new_history(&format!("SELECT {index}"), None),
            )
            .await
            .expect("插入应成功");
            sqlx::query("UPDATE query_history SET started_at = ? WHERE id = ?")
                .bind(1000 + index as i64)
                .bind(&id)
                .execute(database.pool())
                .await
                .expect("更新时间应成功");
        }

        let removed = prune(database.pool(), 3).await.expect("清理应成功");

        assert_eq!(removed, 2);
        assert_eq!(count(database.pool()).await.expect("统计应成功"), 3);
    }

    #[tokio::test]
    async fn clear_removes_all() {
        let database = setup().await;
        insert(database.pool(), &new_history("SELECT 1", None))
            .await
            .expect("插入应成功");

        assert_eq!(clear(database.pool()).await.expect("清空应成功"), 1);
        assert_eq!(count(database.pool()).await.expect("统计应成功"), 0);
    }
}
