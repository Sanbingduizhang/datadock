import { defineStore } from 'pinia';
import { ref } from 'vue';

import { listDatabases, listTables, redisListDatabases } from '@/api/explorer';
import { redisScanKeys } from '@/api/redis';
import { useConnectionStore } from '@/stores/connection';
import { normalizeError } from '@/utils/errors';

import type { DatabaseInfo, TableInfo } from '@/types/database';
import type { RedisKeyInfo } from '@/types/redis';

export type LoadStatus = 'idle' | 'loading' | 'ready' | 'error';

interface Loadable<T> {
  status: LoadStatus;
  items: T[];
  error?: string;
}

function emptyLoadable<T>(): Loadable<T> {
  return { status: 'idle', items: [] };
}

/**
 * 资源树数据（按需加载）。
 * 树结构：连接 → 数据库 → 表/视图。
 */
export const useExplorerStore = defineStore('explorer', () => {
  const connectionStore = useConnectionStore();

  function isRedis(connectionId: string): boolean {
    return (
      connectionStore.connections.find((item) => item.id === connectionId)?.sourceType ===
      'redis'
    );
  }

  /** connectionId -> 数据库列表 */
  const databases = ref<Record<string, Loadable<DatabaseInfo>>>({});
  /** connectionId -> databaseName -> 表/视图列表 */
  const tables = ref<Record<string, Record<string, Loadable<TableInfo>>>>({});
  /** connectionId -> dbIndex -> Redis 键列表（键按库隔离） */
  const redisKeys = ref<Record<string, Record<string, Loadable<RedisKeyInfo>>>>({});

  function databasesState(connectionId: string): Loadable<DatabaseInfo> {
    return databases.value[connectionId] ?? emptyLoadable();
  }

  function tablesState(connectionId: string, database: string): Loadable<TableInfo> {
    return tables.value[connectionId]?.[database] ?? emptyLoadable();
  }

  function redisKeysState(connectionId: string, dbIndex: number): Loadable<RedisKeyInfo> {
    return redisKeys.value[connectionId]?.[String(dbIndex)] ?? emptyLoadable();
  }

  async function loadRedisKeys(
    connectionId: string,
    dbIndex: number,
    force = false,
    pattern?: string,
  ): Promise<void> {
    const current = redisKeysState(connectionId, dbIndex);
    if (!force && (current.status === 'loading' || current.status === 'ready')) {
      return;
    }

    redisKeys.value = {
      ...redisKeys.value,
      [connectionId]: {
        ...(redisKeys.value[connectionId] ?? {}),
        [String(dbIndex)]: { status: 'loading', items: current.items },
      },
    };

    try {
      const response = await redisScanKeys({
        connectionId,
        database: dbIndex,
        pattern: pattern ?? '*',
        count: 200,
      });
      redisKeys.value = {
        ...redisKeys.value,
        [connectionId]: {
          ...(redisKeys.value[connectionId] ?? {}),
          [String(dbIndex)]: { status: 'ready', items: response.keys },
        },
      };
    } catch (raw) {
      const error = normalizeError(raw);
      redisKeys.value = {
        ...redisKeys.value,
        [connectionId]: {
          ...(redisKeys.value[connectionId] ?? {}),
          [String(dbIndex)]: { status: 'error', items: [], error: error.message },
        },
      };
    }
  }

  async function loadDatabases(connectionId: string, force = false): Promise<void> {
    const current = databasesState(connectionId);
    if (!force && (current.status === 'loading' || current.status === 'ready')) {
      return;
    }

    databases.value = {
      ...databases.value,
      [connectionId]: { status: 'loading', items: current.items },
    };

    try {
      const items = isRedis(connectionId)
        ? await redisListDatabases(connectionId)
        : await listDatabases(connectionId);
      databases.value = {
        ...databases.value,
        [connectionId]: { status: 'ready', items },
      };
    } catch (raw) {
      const error = normalizeError(raw);
      databases.value = {
        ...databases.value,
        [connectionId]: { status: 'error', items: [], error: error.message },
      };
    }
  }

  async function loadTables(
    connectionId: string,
    database: string,
    force = false,
  ): Promise<void> {
    const current = tablesState(connectionId, database);
    if (!force && (current.status === 'loading' || current.status === 'ready')) {
      return;
    }

    tables.value = {
      ...tables.value,
      [connectionId]: {
        ...(tables.value[connectionId] ?? {}),
        [database]: { status: 'loading', items: current.items },
      },
    };

    try {
      // Redis 没有表概念：直接标记为空，用户通过「新建查询」使用命令控制台
      const items = isRedis(connectionId)
        ? []
        : await listTables(connectionId, database);
      tables.value = {
        ...tables.value,
        [connectionId]: {
          ...(tables.value[connectionId] ?? {}),
          [database]: { status: 'ready', items },
        },
      };
    } catch (raw) {
      const error = normalizeError(raw);
      tables.value = {
        ...tables.value,
        [connectionId]: {
          ...(tables.value[connectionId] ?? {}),
          [database]: { status: 'error', items: [], error: error.message },
        },
      };
    }
  }

  /** 关闭某个已打开的数据库：清空该库的对象缓存（保留连接） */
  function closeDatabase(connectionId: string, database: string): void {
    const connTables = tables.value[connectionId];
    if (connTables && database in connTables) {
      const next = { ...connTables };
      delete next[database];
      tables.value = { ...tables.value, [connectionId]: next };
    }

    // Redis 按 dbIndex 缓存，库名形如 db0
    const matched = database.match(/^db(\d+)$/i);
    if (matched) {
      const dbIndex = String(matched[1]);
      const connKeys = redisKeys.value[connectionId];
      if (connKeys && dbIndex in connKeys) {
        const next = { ...connKeys };
        delete next[dbIndex];
        redisKeys.value = { ...redisKeys.value, [connectionId]: next };
      }
    }
  }

  /** 连接断开或配置变化后清理缓存 */
  function resetConnection(connectionId: string): void {
    const nextDatabases = { ...databases.value };
    delete nextDatabases[connectionId];

    const nextTables = { ...tables.value };
    delete nextTables[connectionId];

    const nextRedisKeys = { ...redisKeys.value };
    delete nextRedisKeys[connectionId];

    databases.value = nextDatabases;
    tables.value = nextTables;
    redisKeys.value = nextRedisKeys;
  }

  function clear(): void {
    databases.value = {};
    tables.value = {};
    redisKeys.value = {};
  }

  return {
    databases,
    tables,
    redisKeys,
    databasesState,
    tablesState,
    redisKeysState,
    loadDatabases,
    loadTables,
    loadRedisKeys,
    closeDatabase,
    resetConnection,
    clear,
  };
});
