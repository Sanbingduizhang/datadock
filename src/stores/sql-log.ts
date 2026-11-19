import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

/** 触发 SQL 的来源，用于在日志里区分是哪个界面发起的 */
export type SqlLogSource = 'query' | 'table' | 'structure' | 'redis' | 'other';

export type SqlLogStatus = 'success' | 'failed' | 'cancelled';

export interface SqlLogEntry {
  id: string;
  /** 记录时间（Unix 毫秒） */
  at: number;
  sql: string;
  source: SqlLogSource;
  status: SqlLogStatus;
  message: string;
  elapsedMs: number;
}

/** 会话内最多保留的条数；超出后丢弃最旧的记录 */
const MAX_ENTRIES = 200;

let sequence = 0;

/**
 * SQL 操作日志。
 *
 * 只存在内存中：它是"刚才到底执行了什么"的即时回溯工具，
 * 持久化的查询历史由后端 `query_history` 负责，两者不重复。
 */
export const useSqlLogStore = defineStore('sql-log', () => {
  const entries = ref<SqlLogEntry[]>([]);

  const count = computed(() => entries.value.length);
  const failureCount = computed(
    () => entries.value.filter((entry) => entry.status === 'failed').length,
  );

  function push(entry: Omit<SqlLogEntry, 'id' | 'at'> & { at?: number }): void {
    sequence += 1;
    const record: SqlLogEntry = {
      id: `log-${sequence}`,
      at: entry.at ?? Date.now(),
      sql: entry.sql,
      source: entry.source,
      status: entry.status,
      message: entry.message,
      elapsedMs: entry.elapsedMs,
    };

    const next = [record, ...entries.value];
    entries.value = next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
  }

  function clear(): void {
    entries.value = [];
  }

  function remove(id: string): void {
    entries.value = entries.value.filter((entry) => entry.id !== id);
  }

  return { entries, count, failureCount, push, clear, remove };
});

/** 把后端返回的 sourceType 归一成已知来源 */
export function normalizeSource(sourceType?: string | null): SqlLogSource {
  switch ((sourceType ?? '').toLowerCase()) {
    case 'query':
      return 'query';
    case 'table':
      return 'table';
    case 'structure':
      return 'structure';
    case 'redis':
      return 'redis';
    default:
      return 'other';
  }
}
