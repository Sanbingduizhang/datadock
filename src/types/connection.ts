import type { DataSourceType } from '@/types/api';

/** SSL 加密模式（与后端 SslMode 一致） */
export type SslMode =
  | 'disabled'
  | 'prefer'
  | 'require'
  | 'verify-ca'
  | 'verify-full';

/** SSH 隧道认证方式 */
export type SshAuthType = 'password' | 'privateKey';

/** 连接扩展选项：SSL / SSH 隧道（存入 options_json） */
export interface ConnectionOptions {
  sslEnabled: boolean;
  sslMode?: SslMode;
  sshEnabled: boolean;
  sshHost?: string;
  sshPort?: number;
  sshUsername?: string;
  sshAuthType?: SshAuthType;
  sshPrivateKeyPath?: string;
  sshLocalPort?: number;
}

/** 连接配置 + 运行时状态。绝不包含密码。 */
export interface ConnectionInfo {
  id: string;
  name: string;
  sourceType: DataSourceType;
  host: string;
  port: number;
  username: string;
  defaultDatabase?: string | null;
  charset?: string | null;
  /** 运行时是否已连接 */
  connected: boolean;
  serverVersion?: string | null;
  createdAt: number;
  updatedAt: number;
  /** SSL / SSH 隧道等扩展选项 */
  options: ConnectionOptions;
}

export interface CreateConnectionRequest {
  name: string;
  sourceType: DataSourceType;
  host: string;
  port: number;
  username: string;
  password: string;
  defaultDatabase?: string;
  charset?: string;
  connectTimeoutMs?: number;
  /** SSL / SSH 隧道等扩展选项 */
  options?: ConnectionOptions;
}

export interface CreateConnectionResponse {
  connectionId: string;
}

export interface UpdateConnectionRequest {
  connectionId: string;
  name: string;
  host: string;
  port: number;
  username: string;
  defaultDatabase?: string;
  charset?: string;
  connectTimeoutMs?: number;
  /** 不传表示保留原密码 */
  newPassword?: string;
  /** SSL / SSH 隧道等扩展选项 */
  options?: ConnectionOptions;
}

export interface TestConnectionRequest {
  sourceType: DataSourceType;
  host: string;
  port: number;
  username: string;
  password: string;
  defaultDatabase?: string;
  charset?: string;
  connectTimeoutMs?: number;
  /** SSL / SSH 隧道等扩展选项 */
  options?: ConnectionOptions;
}

export interface TestConnectionResponse {
  success: boolean;
  message: string;
  serverVersion?: string | null;
  elapsedMs: number;
}

export interface ConnectDatabaseResponse {
  connectionId: string;
  status: 'connected';
  serverVersion?: string | null;
  currentDatabase?: string | null;
}

export interface DisconnectDatabaseResponse {
  connectionId: string;
  status: 'disconnected';
}

export interface ConnectionRuntimeInfo {
  connectionId: string;
  status: 'disconnected' | 'connecting' | 'connected' | 'error';
  currentDatabase?: string | null;
  connectedAt?: number | null;
  lastError?: string | null;
}

export interface SimpleSuccessResponse {
  success: boolean;
}

/** 单条连接的导出条目（不含密码） */
export interface ConnectionExportEntry {
  name: string;
  sourceType: DataSourceType;
  host: string;
  port: number;
  username: string;
  defaultDatabase?: string | null;
  charset?: string | null;
  connectTimeoutMs?: number | null;
  options: ConnectionOptions;
}

/** 连接配置导出文件结构 */
export interface ConnectionExportFile {
  format: string;
  version: number;
  exportedAt: number;
  connections: ConnectionExportEntry[];
}

/** 连接导入结果 */
export interface ConnectionImportResponse {
  imported: number;
  skipped: number;
}
