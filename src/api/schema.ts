import { invokeCommand } from './index';

import type { DatabaseDdl, TableSchema } from '@/types/database';

export function getTableSchema(
  connectionId: string,
  databaseName: string,
  tableName: string,
): Promise<TableSchema> {
  return invokeCommand<TableSchema>('get_table_schema', {
    request: { connectionId, databaseName, tableName },
  });
}

/** 读取建库语句（SHOW CREATE DATABASE） */
export function getDatabaseDdl(
  connectionId: string,
  databaseName: string,
): Promise<DatabaseDdl> {
  return invokeCommand<DatabaseDdl>('get_database_ddl', {
    request: { connectionId, databaseName },
  });
}
