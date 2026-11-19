/** SQL 执行结果类型，与 src-tauri/src/models/query.rs 一致 */

export type DatabaseValueType =
  | 'null'
  | 'integer'
  | 'decimal'
  | 'float'
  | 'boolean'
  | 'string'
  | 'datetime'
  | 'json'
  | 'binary';

export interface DatabaseValue {
  type: DatabaseValueType | string;
  value?: string | null;
}

export interface QueryColumn {
  name: string;
  typeName: string;
}

export interface QueryResultSet {
  columns: QueryColumn[];
  rows: DatabaseValue[][];
  rowCount: number;
  truncated: boolean;
}

/** 单条语句的执行结果 */
export interface StatementResult {
  index: number;
  sqlPreview: string;
  resultSet: QueryResultSet | null;
  affectedRows: number | null;
  lastInsertId: string | null;
  elapsedMs: number;
  message: string;
}

export interface StatementError {
  index: number;
  sqlPreview: string;
  message: string;
}

export interface ExecuteSqlRequest {
  connectionId: string;
  databaseName?: string;
  sql: string;
  maxRows?: number;
  /** 前端生成的执行 ID，用于取消 */
  taskId?: string;
  sourceType?: string;
}

export interface ExecuteSqlResponse {
  taskId: string;
  results: StatementResult[];
  /** 首个失败语句；null 表示全部成功 */
  error: StatementError | null;
  elapsedMs: number;
  message: string;
  truncated: boolean;
  cancelled: boolean;
  historyId: string | null;
}

export interface ExecuteBatchResponse {
  results: StatementResult[];
  error: StatementError | null;
  /** 是否因失败而回滚 */
  rolledBack: boolean;
  elapsedMs: number;
  message: string;
  historyId: string | null;
}

export interface CancelTaskResponse {
  cancelled: boolean;
}
