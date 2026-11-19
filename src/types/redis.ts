/** Redis 键浏览类型，与 src-tauri/src/models/redis.rs 一致 */

export type RedisKeyType =
  | 'string'
  | 'hash'
  | 'list'
  | 'set'
  | 'zset'
  | 'stream'
  | 'none';

export interface RedisKeyInfo {
  name: string;
  type: RedisKeyType;
  ttlSeconds: number;
}

export interface RedisScanRequest {
  connectionId: string;
  /** 目标库索引；缺省时用连接当前库，传入可在指定库上扫描（切换库浏览） */
  database?: number;
  pattern?: string;
  count?: number;
  includeSystemKeys?: boolean;
}

export interface RedisScanResponse {
  keys: RedisKeyInfo[];
  cursor: number;
}

export interface RedisKeyValueRequest {
  connectionId: string;
  /** 目标库索引；缺省时用连接当前库，传入可在指定库上读取键值 */
  database?: number;
  keyName: string;
  maxItems?: number;
}

export interface HashField {
  field: string;
  value: string;
}

export interface ListItem {
  index: number;
  value: string;
}

export interface ZsetEntry {
  member: string;
  score: number;
}

export interface StreamEntry {
  id: string;
  fields: HashField[];
}

/** 键值内容：根据类型展示不同形状 */
export type RedisKeyValue =
  | { kind: 'string'; value: string | null }
  | { kind: 'hash'; fields: HashField[] }
  | { kind: 'list'; items: ListItem[] }
  | { kind: 'set'; members: string[] }
  | { kind: 'zset'; entries: ZsetEntry[] }
  | { kind: 'stream'; entries: StreamEntry[] }
  | { kind: 'none' };

export interface RedisKeyValueResponse {
  keyName: string;
  type: RedisKeyType;
  ttlSeconds: number;
  value: RedisKeyValue;
}
