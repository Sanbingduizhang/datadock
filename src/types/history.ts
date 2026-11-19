/** 查询历史类型，与 src-tauri/src/models/history.rs 一致 */

export type QueryHistoryStatus = 'running' | 'success' | 'failed' | 'cancelled';

export interface QueryHistoryEntry {
  id: string;
  connectionId: string | null;
  sourceType: string;
  databaseName: string | null;
  sqlText: string;
  status: QueryHistoryStatus | string;
  returnedRows: number;
  affectedRows: number | null;
  elapsedMs: number;
  errorMessage: string | null;
  /** Unix 毫秒时间戳 */
  startedAt: number;
  finishedAt: number | null;
}

export interface ListQueryHistoryRequest {
  connectionId?: string;
  keyword?: string;
  limit?: number;
  offset?: number;
}

export interface ListQueryHistoryResponse {
  items: QueryHistoryEntry[];
  total: number;
}
