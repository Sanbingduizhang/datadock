export type WorkspaceTabType =
  | 'welcome'
  | 'query'
  | 'table'
  | 'structure'
  | 'redis-key'
  | 'redis-db';

/** 工作区标签的数据源上下文，切换标签时不能丢失或串用 */
export interface WorkspaceTabContext {
  connectionId?: string;
  connectionName?: string;
  databaseName?: string;
  tableName?: string;
  /** Redis 键名（redis-key 标签使用） */
  keyName?: string;
  /** Redis 库索引（redis-db / redis-key 标签使用） */
  redisDbIndex?: number;
}

export interface WorkspaceTab {
  id: string;
  type: WorkspaceTabType;
  title: string;
  context: WorkspaceTabContext;
  /** 是否允许关闭（欢迎页不允许关闭） */
  closable: boolean;
  /** 是否存在未保存内容 */
  dirty: boolean;
  /** SQL 查询标签的临时编辑内容（阶段 E 交由 Monaco 管理） */
  draft?: string;
}

/* ---------- 持久化结构（与 src-tauri/src/models/workspace.rs 一致） ---------- */

export interface WorkspaceLayoutState {
  sidebarWidth: number;
  sidebarVisible: boolean;
  resultPanelHeight: number;
  resultPanelVisible: boolean;
  statusBarVisible: boolean;
}

export interface PersistedWorkspaceTab {
  id: string;
  type: WorkspaceTabType;
  title: string;
  context: WorkspaceTabContext;
  draft?: string;
}

export interface PersistedWorkspaceState {
  version: number;
  /** 工作区显示名（多工作区切换时展示） */
  name?: string;
  activeTabId?: string;
  tabs: PersistedWorkspaceTab[];
  layout: WorkspaceLayoutState;
}

export interface GetWorkspaceRequest {
  workspaceId: string;
}

export interface GetWorkspaceResponse {
  workspaceId: string;
  state: PersistedWorkspaceState | null;
}

export interface SaveWorkspaceRequest {
  workspaceId: string;
  state: PersistedWorkspaceState;
}

export interface SaveWorkspaceResponse {
  success: boolean;
}

/* ---------- 多工作区 ---------- */

export interface WorkspaceSummary {
  id: string;
  name: string;
  /** 最近保存时间（Unix 毫秒） */
  updatedAt: number;
  tabCount: number;
}

export interface DeleteWorkspaceRequest {
  workspaceId: string;
}
