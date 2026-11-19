import { invokeCommand } from './index';

import type {
  ListQueryHistoryRequest,
  ListQueryHistoryResponse,
} from '@/types/history';

/** 分页读取查询历史（按开始时间倒序） */
export function listQueryHistory(
  request: ListQueryHistoryRequest,
): Promise<ListQueryHistoryResponse> {
  return invokeCommand<ListQueryHistoryResponse>('list_query_history', { request });
}

/** 删除单条历史 */
export function deleteQueryHistory(historyId: string): Promise<boolean> {
  return invokeCommand<boolean>('delete_query_history', { request: { historyId } });
}

/** 清空历史；不传连接 ID 时清空全部 */
export function clearQueryHistory(connectionId?: string): Promise<number> {
  return invokeCommand<number>('clear_query_history', {
    request: { connectionId: connectionId ?? null },
  });
}
