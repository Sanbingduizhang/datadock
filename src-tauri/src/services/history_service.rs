//! 查询历史业务：列表、删除、清空。
//!
//! 历史记录由 `QueryService` 在执行时写入，这里只负责读取与维护。

use crate::database::Database;
use crate::errors::AppError;
use crate::models::history::{
    ListQueryHistoryRequest, ListQueryHistoryResponse, QueryHistoryEntry,
};
use crate::repositories::history_repository;

pub struct HistoryService {
    database: Database,
}

impl HistoryService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list(
        &self,
        request: ListQueryHistoryRequest,
    ) -> Result<ListQueryHistoryResponse, AppError> {
        let items = history_repository::list(
            self.database.pool(),
            request.normalized_limit(),
            request.normalized_offset(),
            request.connection_id.as_deref(),
            request.keyword.as_deref(),
        )
        .await?;

        let total = history_repository::count(self.database.pool()).await?;

        Ok(ListQueryHistoryResponse { items, total })
    }

    pub async fn get(&self, id: &str) -> Result<Option<QueryHistoryEntry>, AppError> {
        history_repository::get(self.database.pool(), id).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool, AppError> {
        history_repository::delete(self.database.pool(), id).await
    }

    pub async fn clear(&self, connection_id: Option<&str>) -> Result<u64, AppError> {
        history_repository::clear_for_connection(self.database.pool(), connection_id).await
    }
}
