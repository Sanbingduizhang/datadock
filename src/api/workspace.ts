import { invokeCommand } from './index';

import type {
  DeleteWorkspaceRequest,
  GetWorkspaceRequest,
  GetWorkspaceResponse,
  PersistedWorkspaceState,
  SaveWorkspaceRequest,
  SaveWorkspaceResponse,
  WorkspaceSummary,
} from '@/types/workspace';

/** 读取工作区状态；首次启动 state 为 null */
export function getWorkspace(workspaceId = 'main'): Promise<GetWorkspaceResponse> {
  const request: GetWorkspaceRequest = { workspaceId };
  return invokeCommand<GetWorkspaceResponse>('get_workspace', { request });
}

/** 保存工作区状态（后端校验标签数量与标题长度） */
export function saveWorkspace(
  state: PersistedWorkspaceState,
  workspaceId = 'main',
): Promise<SaveWorkspaceResponse> {
  const request: SaveWorkspaceRequest = { workspaceId, state };
  return invokeCommand<SaveWorkspaceResponse>('save_workspace', { request });
}

/** 列出全部工作区（按最近保存时间倒序） */
export function listWorkspaces(): Promise<WorkspaceSummary[]> {
  return invokeCommand<WorkspaceSummary[]>('list_workspaces');
}

/** 删除工作区（默认工作区不可删除） */
export function deleteWorkspace(workspaceId: string): Promise<boolean> {
  const request: DeleteWorkspaceRequest = { workspaceId };
  return invokeCommand<boolean>('delete_workspace', { request });
}
