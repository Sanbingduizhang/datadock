//! 表数据浏览业务。

use std::sync::Arc;

use crate::adapters::mysql::export as mysql_export;
use crate::adapters::mysql::table as mysql_table;
use crate::adapters::pg::export as pg_export;
use crate::adapters::pg::table as pg_table;
use crate::core::{ConnectionManager, PgManager};
use crate::errors::{error_code, AppError};
use crate::models::export::{ExportResponse, ExportTableRequest};
use crate::models::table::{TableDataRequest, TableDataResponse};

pub struct TableService {
    manager: Arc<ConnectionManager>,
    pg_manager: Arc<PgManager>,
}

impl TableService {
    pub fn new(manager: Arc<ConnectionManager>, pg_manager: Arc<PgManager>) -> Self {
        Self {
            manager,
            pg_manager,
        }
    }

    pub async fn data(&self, request: TableDataRequest) -> Result<TableDataResponse, AppError> {
        let filter = validated_filter(request.filter.as_deref())?;
        let order_desc = request.order_desc.unwrap_or(false);

        // PostgreSQL：`databaseName` 即 schema 名，复用连接级长连接池
        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            return pg_table::fetch_data(
                &pg.pool,
                &request.database_name,
                &request.table_name,
                request.normalized_page(),
                request.normalized_page_size(),
                request.order_by.as_deref(),
                order_desc,
                filter.as_deref(),
            )
            .await;
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let pool = connection.pool_for(Some(&request.database_name)).await?;

        mysql_table::fetch_data(
            &pool,
            &request.database_name,
            &request.table_name,
            request.normalized_page(),
            request.normalized_page_size(),
            request.order_by.as_deref(),
            order_desc,
            filter.as_deref(),
        )
        .await
    }

    pub async fn export(&self, request: ExportTableRequest) -> Result<ExportResponse, AppError> {
        let filter = validated_filter(request.filter.as_deref())?;
        let order_desc = request.order_desc.unwrap_or(false);

        if let Some(pg) = self.pg_manager.get(&request.connection_id).await {
            return pg_export::export_table(
                &pg.pool,
                &request.database_name,
                &request.table_name,
                request.format,
                &request.path,
                request.order_by.as_deref(),
                order_desc,
                filter.as_deref(),
            )
            .await;
        }

        let connection = self
            .manager
            .get(&request.connection_id)
            .await
            .ok_or_else(|| {
                AppError::new(
                    error_code::CONNECTION_NOT_READY,
                    "连接尚未建立，请先连接数据库",
                )
            })?;

        let pool = connection.pool_for(Some(&request.database_name)).await?;

        mysql_export::export_table(
            &pool,
            &request.database_name,
            &request.table_name,
            request.format,
            &request.path,
            request.order_by.as_deref(),
            order_desc,
            filter.as_deref(),
        )
        .await
    }
}

/// 基础校验：只允许单条表达式，避免把分号带进拼接的 SQL。
fn validated_filter(filter: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if filter.chars().count() > 2_000 {
        return Err(AppError::validation("筛选条件过长（最多 2000 字符）"));
    }
    if filter.contains(';') {
        return Err(AppError::validation("筛选条件中不能包含分号"));
    }

    Ok(Some(filter.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_rejects_semicolon() {
        assert!(validated_filter(Some("id = 1; DROP TABLE users")).is_err());
    }

    #[test]
    fn filter_blank_becomes_none() {
        assert!(validated_filter(Some("   "))
            .expect("空条件应通过")
            .is_none());
        assert!(validated_filter(None).expect("未传条件应通过").is_none());
    }

    #[test]
    fn filter_is_trimmed() {
        assert_eq!(
            validated_filter(Some(" status = 1 ")).expect("应通过"),
            Some("status = 1".to_string())
        );
    }
}
