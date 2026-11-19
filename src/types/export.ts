/** 数据导出类型，与 src-tauri/src/models/export.rs 一致 */

export type ExportFormat = 'csv' | 'json';

export interface ExportTableRequest {
  connectionId: string;
  databaseName: string;
  tableName: string;
  format: ExportFormat;
  path: string;
  filter?: string;
  orderBy?: string;
  orderDesc?: boolean;
}

export interface ExportQueryRequest {
  connectionId: string;
  databaseName?: string;
  sql: string;
  format: ExportFormat;
  path: string;
  maxRows?: number;
}

export interface ExportResponse {
  path: string;
  rowsWritten: number;
  elapsedMs: number;
  truncated: boolean;
}

/** 数据库级导出内容：结构 / 数据 / 结构+数据 */
export type DumpMode = 'structure' | 'data' | 'both';

export interface DumpDatabaseRequest {
  connectionId: string;
  databaseName: string;
  /** 空表示全部表；否则仅导出指定表 */
  tables?: string[];
  mode: DumpMode;
  /** 目标 SQL 文件绝对路径 */
  path: string;
}

export interface DumpDatabaseResponse {
  path: string;
  tables: number;
  rowsWritten: number;
  elapsedMs: number;
}

export interface ImportDatabaseRequest {
  connectionId: string;
  /** 目标库（MySQL 用） */
  databaseName?: string;
  /** 源 SQL 文件绝对路径 */
  path: string;
}

export interface ImportDatabaseResponse {
  statements: number;
  elapsedMs: number;
}
