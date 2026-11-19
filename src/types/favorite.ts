/** SQL 收藏类型，与 src-tauri/src/models/favorite.rs 一致 */

export interface SqlFavorite {
  id: string;
  name: string;
  connectionId: string | null;
  databaseName: string | null;
  sqlText: string;
  category: string | null;
  remark: string | null;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface SqlFavoriteInput {
  name: string;
  connectionId?: string | null;
  databaseName?: string | null;
  sqlText: string;
  category?: string | null;
  remark?: string | null;
}
