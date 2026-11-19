/**
 * 前后端公共类型定义。
 * 与 src-tauri/src/models 中的结构保持一致（Serde 统一序列化为 camelCase）。
 */

/** 统一错误结构，前后端所有失败结果都使用该形状 */
export interface AppError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export type DataSourceType = 'mysql' | 'redis' | 'postgres';

export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'disconnecting' | 'error';

export type QueryTaskStatus = 'pending' | 'running' | 'success' | 'failed' | 'cancelled';

/* ---------- 应用基础信息（阶段 A 用于验证 IPC 通道） ---------- */

export interface PingRequest {
  message?: string;
}

export interface PingResponse {
  reply: string;
  receivedAtMs: number;
}

export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
  os: string;
  arch: string;
  dataDir: string;
  logDir: string;
  startedAtMs: number;
}
