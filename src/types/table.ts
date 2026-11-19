/** 表数据浏览类型，与 src-tauri/src/models/table.rs 一致 */

import type { DatabaseValue, QueryColumn } from './query';

export interface TableDataRequest {
  connectionId: string;
  databaseName: string;
  tableName: string;
  page?: number;
  pageSize?: number;
  orderBy?: string;
  orderDesc?: boolean;
  /** WHERE 之后的表达式，例如 status = 1 */
  filter?: string;
}

export interface TableDataResponse {
  columns: QueryColumn[];
  rows: DatabaseValue[][];
  totalRows: number;
  page: number;
  pageSize: number;
  elapsedMs: number;
  primaryKey: string[];
}
