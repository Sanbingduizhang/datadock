import { invokeCommand } from './index';

import type {
  ConnectDatabaseResponse,
  ConnectionExportFile,
  ConnectionImportResponse,
  ConnectionInfo,
  ConnectionRuntimeInfo,
  CreateConnectionRequest,
  CreateConnectionResponse,
  DisconnectDatabaseResponse,
  SimpleSuccessResponse,
  TestConnectionRequest,
  TestConnectionResponse,
  UpdateConnectionRequest,
} from '@/types/connection';

export function listConnections(): Promise<ConnectionInfo[]> {
  return invokeCommand<ConnectionInfo[]>('list_connections');
}

export function createConnection(
  request: CreateConnectionRequest,
): Promise<CreateConnectionResponse> {
  return invokeCommand<CreateConnectionResponse>('create_connection', { request });
}

export function updateConnection(
  request: UpdateConnectionRequest,
): Promise<SimpleSuccessResponse> {
  return invokeCommand<SimpleSuccessResponse>('update_connection', { request });
}

export function deleteConnection(connectionId: string): Promise<SimpleSuccessResponse> {
  return invokeCommand<SimpleSuccessResponse>('delete_connection', {
    request: { connectionId },
  });
}

export function testConnection(
  request: TestConnectionRequest,
): Promise<TestConnectionResponse> {
  return invokeCommand<TestConnectionResponse>('test_connection', { request });
}

export function connectDatabase(connectionId: string): Promise<ConnectDatabaseResponse> {
  return invokeCommand<ConnectDatabaseResponse>('connect_database', {
    request: { connectionId },
  });
}

export function disconnectDatabase(connectionId: string): Promise<DisconnectDatabaseResponse> {
  return invokeCommand<DisconnectDatabaseResponse>('disconnect_database', {
    request: { connectionId },
  });
}

export function getConnectionStatus(connectionId: string): Promise<ConnectionRuntimeInfo> {
  return invokeCommand<ConnectionRuntimeInfo>('get_connection_status', {
    request: { connectionId },
  });
}

/** 导出全部连接配置（JSON，不含密码） */
export function exportConnections(): Promise<ConnectionExportFile> {
  return invokeCommand<ConnectionExportFile>('export_connections');
}

/** 导入连接配置：JSON 文本，按名称去重，返回导入与跳过数量 */
export function importConnections(json: string): Promise<ConnectionImportResponse> {
  return invokeCommand<ConnectionImportResponse>('import_connections', { json });
}
