import { invokeCommand } from './index';

import { normalizeSource, useSqlLogStore } from '@/stores/sql-log';

import type {
  CancelTaskResponse,
  ExecuteBatchResponse,
  ExecuteSqlRequest,
  ExecuteSqlResponse,
} from '@/types/query';

/**
 * 执行 SQL（支持多语句，按顺序执行并在首个错误处停止）。
 *
 * 所有经由这里发起的执行都会写入「SQL 日志」，便于在界面任意位置操作后
 * 回看实际发送的语句。
 */
export async function executeSql(request: ExecuteSqlRequest): Promise<ExecuteSqlResponse> {
  const startedAt = Date.now();
  const log = useSqlLogStore();

  try {
    const response = await invokeCommand<ExecuteSqlResponse>('execute_sql', { request });
    log.push({
      sql: request.sql,
      source: normalizeSource(request.sourceType),
      status: response.cancelled ? 'cancelled' : response.error ? 'failed' : 'success',
      message: response.error ? response.error.message : response.message,
      elapsedMs: response.elapsedMs || Date.now() - startedAt,
    });
    return response;
  } catch (error) {
    log.push({
      sql: request.sql,
      source: normalizeSource(request.sourceType),
      status: 'failed',
      message: error instanceof Error ? error.message : String(error),
      elapsedMs: Date.now() - startedAt,
    });
    throw error;
  }
}

/** 取消正在执行的查询（服务端 KILL QUERY） */
export function cancelTask(taskId: string): Promise<CancelTaskResponse> {
  return invokeCommand<CancelTaskResponse>('cancel_task', { request: { taskId } });
}

/** 批量执行：整体事务包裹，任一语句失败则全部回滚 */
export async function executeBatchSql(
  request: ExecuteSqlRequest,
): Promise<ExecuteBatchResponse> {
  const startedAt = Date.now();
  const log = useSqlLogStore();

  try {
    const response = await invokeCommand<ExecuteBatchResponse>('execute_batch_sql', { request });
    log.push({
      sql: request.sql,
      source: normalizeSource(request.sourceType),
      status: response.error ? 'failed' : 'success',
      message: response.message,
      elapsedMs: response.elapsedMs || Date.now() - startedAt,
    });
    return response;
  } catch (error) {
    log.push({
      sql: request.sql,
      source: normalizeSource(request.sourceType),
      status: 'failed',
      message: error instanceof Error ? error.message : String(error),
      elapsedMs: Date.now() - startedAt,
    });
    throw error;
  }
}

/** Redis 命令控制台：整行按命令解析执行 */
export async function redisExecute(
  connectionId: string,
  command: string,
): Promise<ExecuteSqlResponse> {
  const startedAt = Date.now();
  const log = useSqlLogStore();

  try {
    const response = await invokeCommand<ExecuteSqlResponse>('redis_execute', {
      request: { connectionId, command },
    });
    log.push({
      sql: command,
      source: 'redis',
      status: response.error ? 'failed' : 'success',
      message: response.error ? response.error.message : response.message,
      elapsedMs: response.elapsedMs || Date.now() - startedAt,
    });
    return response;
  } catch (error) {
    log.push({
      sql: command,
      source: 'redis',
      status: 'failed',
      message: error instanceof Error ? error.message : String(error),
      elapsedMs: Date.now() - startedAt,
    });
    throw error;
  }
}
