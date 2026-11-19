import { invokeCommand } from './index';

import type { SqlFavorite, SqlFavoriteInput } from '@/types/favorite';

/** 读取全部 SQL 收藏（按排序字段） */
export function listSqlFavorites(): Promise<SqlFavorite[]> {
  return invokeCommand<SqlFavorite[]>('list_sql_favorites');
}

/** 新建收藏，返回新记录 ID */
export function createSqlFavorite(request: SqlFavoriteInput): Promise<string> {
  return invokeCommand<string>('create_sql_favorite', { request });
}

/** 更新收藏内容 */
export function updateSqlFavorite(
  favoriteId: string,
  request: SqlFavoriteInput,
): Promise<boolean> {
  return invokeCommand<boolean>('update_sql_favorite', {
    request: { favoriteId, ...request },
  });
}

export function deleteSqlFavorite(favoriteId: string): Promise<boolean> {
  return invokeCommand<boolean>('delete_sql_favorite', { request: { favoriteId } });
}
