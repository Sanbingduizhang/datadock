import type {
  PersistedWorkspaceState,
  PersistedWorkspaceTab,
  WorkspaceLayoutState,
  WorkspaceTab,
  WorkspaceTabType,
} from '@/types/workspace';

/** 工作区状态的持久化版本，结构不兼容变化时递增 */
export const WORKSPACE_STATE_VERSION = 1;

/** 单窗口应用，固定使用一个状态键 */
export const WORKSPACE_STORAGE_KEY = 'main';

/** 编辑器内容最多持久化的字符数，避免异常数据撑大 SQLite 记录 */
const MAX_DRAFT_LENGTH = 200_000;

const TAB_TYPES: readonly WorkspaceTabType[] = [
  'welcome',
  'query',
  'table',
  'structure',
  'redis-key',
  'redis-db',
];

export function isSupportedWorkspaceVersion(version: number): boolean {
  return version === WORKSPACE_STATE_VERSION;
}

/** 由当前 UI 状态构造待持久化的结构 */
export function buildPersistedState(input: {
  layout: WorkspaceLayoutState;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
  name?: string;
}): PersistedWorkspaceState {
  const tabs: PersistedWorkspaceTab[] = input.tabs.map((tab) => {
    const persisted: PersistedWorkspaceTab = {
      id: tab.id,
      type: tab.type,
      title: tab.title,
      context: { ...tab.context },
    };

    if (tab.type === 'query' && tab.draft) {
      persisted.draft = tab.draft.slice(0, MAX_DRAFT_LENGTH);
    }

    return persisted;
  });

  const state: PersistedWorkspaceState = {
    version: WORKSPACE_STATE_VERSION,
    tabs,
    layout: { ...input.layout },
  };

  if (input.name) {
    state.name = input.name;
  }

  if (input.activeTabId) {
    state.activeTabId = input.activeTabId;
  }

  return state;
}

/**
 * 还原为工作区标签。
 * 恢复的 SQL 内容若存在，标记为未保存，避免用户误以为已经保存到文件。
 */
export function toWorkspaceTabs(state: PersistedWorkspaceState): WorkspaceTab[] {
  return state.tabs
    .filter((tab) => TAB_TYPES.includes(tab.type) && tab.id.length > 0)
    .map<WorkspaceTab>((tab) => {
      const draft = tab.type === 'query' ? (tab.draft ?? '') : undefined;

      return {
        id: tab.id,
        type: tab.type,
        title: tab.title,
        context: { ...tab.context },
        closable: tab.type !== 'welcome',
        dirty: Boolean(draft && draft.length > 0),
        ...(draft === undefined ? {} : { draft }),
      };
    });
}
