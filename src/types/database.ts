/** 数据库对象类型，与 src-tauri/src/models/database.rs 一致 */

export interface DatabaseInfo {
  name: string;
}

export type TableKind = 'table' | 'view';

export interface TableInfo {
  name: string;
  type: TableKind;
  engine?: string | null;
  comment?: string | null;
}

export interface ColumnInfo {
  name: string;
  /** 完整类型，例如 varchar(64) */
  columnType: string;
  nullable: boolean;
  defaultValue?: string | null;
  columnKey: string;
  extra: string;
  comment: string;
  position: number;
}

export interface IndexInfo {
  name: string;
  unique: boolean;
  indexType: string;
  columns: string[];
}

export interface TableSchema {
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  ddl: string;
}

export interface DatabaseDdl {
  databaseName: string;
  ddl: string;
}
