import { invokeCommand } from './index';

import { normalizeSource, useSqlLogStore } from '@/stores/sql-log';

import type {
  RedisKeyValue,
  RedisKeyValueRequest,
  RedisKeyValueResponse,
  RedisKeyType,
  RedisScanRequest,
  RedisScanResponse,
} from '@/types/redis';

/** 扫描 Redis 键（SCAN 游标迭代） */
export function redisScanKeys(request: RedisScanRequest): Promise<RedisScanResponse> {
  return invokeCommand<RedisScanResponse>('redis_scan_keys', { request });
}

/** 读取 Redis 键值内容 */
export function redisGetKeyValue(request: RedisKeyValueRequest): Promise<RedisKeyValueResponse> {
  return invokeCommand<RedisKeyValueResponse>('redis_get_key_value', { request });
}

/** 删除键 */
export function redisDeleteKey(connectionId: string, database: number, keyName: string): Promise<{ success: boolean }> {
  return invokeCommand('redis_delete_key', {
    request: { connectionId, database, keyName },
  });
}

/** 重命名键 */
export function redisRenameKey(
  connectionId: string,
  database: number,
  from: string,
  to: string,
): Promise<{ success: boolean }> {
  return invokeCommand('redis_rename_key', {
    request: { connectionId, database, from, to },
  });
}

/** 设置过期时间（秒；<=0 表示永不过期） */
export function redisSetTtl(
  connectionId: string,
  database: number,
  keyName: string,
  ttlSeconds: number,
): Promise<{ success: boolean }> {
  return invokeCommand('redis_set_ttl', {
    request: { connectionId, database, keyName, ttlSeconds },
  });
}

/** 写入键值（编辑或新增） */
export async function redisSetKeyValue(
  connectionId: string,
  database: number,
  keyName: string,
  keyType: RedisKeyType,
  value: RedisKeyValue,
  ttlSeconds?: number,
): Promise<{ success: boolean }> {
  const startedAt = Date.now();
  const log = useSqlLogStore();
  const sql = `# 写入 ${keyType} 键 ${keyName}`;
  try {
    const response = await invokeCommand<{ success: boolean }>('redis_set_key_value', {
      request: { connectionId, database, keyName, keyType, value, ttlSeconds },
    });
    log.push({
      sql,
      source: normalizeSource('redis'),
      status: 'success',
      message: `已写入键 ${keyName}`,
      elapsedMs: Date.now() - startedAt,
    });
    return response;
  } catch (error) {
    log.push({
      sql,
      source: normalizeSource('redis'),
      status: 'failed',
      message: error instanceof Error ? error.message : String(error),
      elapsedMs: Date.now() - startedAt,
    });
    throw error;
  }
}
