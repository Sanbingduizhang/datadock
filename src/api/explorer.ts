import { invokeCommand } from './index';

import type { DatabaseInfo, TableInfo } from '@/types/database';

export function listDatabases(connectionId: string): Promise<DatabaseInfo[]> {
  return invokeCommand<DatabaseInfo[]>('list_databases', {
    request: { connectionId },
  });
}

export function listTables(connectionId: string, databaseName: string): Promise<TableInfo[]> {
  return invokeCommand<TableInfo[]>('list_tables', {
    request: { connectionId, databaseName },
  });
}

/** Redis 只返回当前配置的库 */
export function redisListDatabases(connectionId: string): Promise<DatabaseInfo[]> {
  return invokeCommand<DatabaseInfo[]>('redis_list_databases', {
    request: { connectionId },
  });
}
