import { describe, expect, it } from 'vitest';

import {
  buildPersistedState,
  isSupportedWorkspaceVersion,
  toWorkspaceTabs,
  WORKSPACE_STATE_VERSION,
} from '@/utils/workspace-persist';

import type { WorkspaceLayoutState, WorkspaceTab } from '@/types/workspace';

const LAYOUT: WorkspaceLayoutState = {
  sidebarWidth: 300,
  sidebarVisible: true,
  resultPanelHeight: 200,
  resultPanelVisible: false,
  statusBarVisible: true,
};

function tab(overrides: Partial<WorkspaceTab> = {}): WorkspaceTab {
  return {
    id: 'tab-1',
    type: 'query',
    title: 'SQL 查询 1',
    context: {},
    closable: true,
    dirty: false,
    draft: 'SELECT 1;',
    ...overrides,
  };
}

describe('buildPersistedState', () => {
  it('只保留持久化需要的字段', () => {
    const state = buildPersistedState({
      layout: LAYOUT,
      tabs: [tab()],
      activeTabId: 'tab-1',
    });

    expect(state.version).toBe(WORKSPACE_STATE_VERSION);
    expect(state.activeTabId).toBe('tab-1');
    expect(state.layout).toEqual(LAYOUT);
    expect(state.tabs[0]).toEqual({
      id: 'tab-1',
      type: 'query',
      title: 'SQL 查询 1',
      context: {},
      draft: 'SELECT 1;',
    });
    expect(state.tabs[0]).not.toHaveProperty('dirty');
    expect(state.tabs[0]).not.toHaveProperty('closable');
  });

  it('没有激活标签时不写入 activeTabId 字段', () => {
    const state = buildPersistedState({ layout: LAYOUT, tabs: [], activeTabId: null });

    expect(state).not.toHaveProperty('activeTabId');
  });

  it('非查询标签不保存 draft', () => {
    const state = buildPersistedState({
      layout: LAYOUT,
      tabs: [tab({ id: 'tab-2', type: 'table', title: 'users', draft: 'ignored' })],
      activeTabId: 'tab-2',
    });

    expect(state.tabs[0]).not.toHaveProperty('draft');
  });

  it('超长草稿会被截断，避免异常数据撑大记录', () => {
    const longDraft = 'x'.repeat(250_000);
    const state = buildPersistedState({
      layout: LAYOUT,
      tabs: [tab({ draft: longDraft })],
      activeTabId: 'tab-1',
    });

    expect(state.tabs[0]?.draft?.length).toBe(200_000);
  });

  it('上下文被复制而非引用', () => {
    const context = { connectionId: 'conn-1', connectionName: '本地' };
    const state = buildPersistedState({
      layout: LAYOUT,
      tabs: [tab({ context })],
      activeTabId: 'tab-1',
    });

    state.tabs[0]!.context.connectionId = 'changed';
    expect(context.connectionId).toBe('conn-1');
  });
});

describe('toWorkspaceTabs', () => {
  it('恢复草稿并标记未保存，欢迎页不可关闭', () => {
    const tabs = toWorkspaceTabs({
      version: WORKSPACE_STATE_VERSION,
      activeTabId: 'tab-1',
      tabs: [
        { id: 'tab-0', type: 'welcome', title: '欢迎', context: {} },
        { id: 'tab-1', type: 'query', title: 'SQL 查询 1', context: {}, draft: 'SELECT 1;' },
      ],
      layout: LAYOUT,
    });

    expect(tabs).toHaveLength(2);
    expect(tabs[0]?.closable).toBe(false);
    expect(tabs[1]?.closable).toBe(true);
    expect(tabs[1]?.dirty).toBe(true);
    expect(tabs[1]?.draft).toBe('SELECT 1;');
  });

  it('空草稿不标记未保存', () => {
    const tabs = toWorkspaceTabs({
      version: WORKSPACE_STATE_VERSION,
      tabs: [{ id: 'tab-1', type: 'query', title: 'SQL 查询 1', context: {}, draft: '' }],
      layout: LAYOUT,
    });

    expect(tabs[0]?.dirty).toBe(false);
  });

  it('过滤非法类型与空 ID 的记录', () => {
    const tabs = toWorkspaceTabs({
      version: WORKSPACE_STATE_VERSION,
      tabs: [
        { id: '', type: 'query', title: '空 ID', context: {} },
        // 模拟旧版本或被手工修改的脏数据
        { id: 'tab-x', type: 'unknown' as never, title: '非法类型', context: {} },
        { id: 'tab-1', type: 'table', title: 'users', context: { tableName: 'users' } },
      ],
      layout: LAYOUT,
    });

    expect(tabs).toHaveLength(1);
    expect(tabs[0]?.id).toBe('tab-1');
    expect(tabs[0]?.type).toBe('table');
  });
});

describe('isSupportedWorkspaceVersion', () => {
  it('只接受当前版本', () => {
    expect(isSupportedWorkspaceVersion(WORKSPACE_STATE_VERSION)).toBe(true);
    expect(isSupportedWorkspaceVersion(WORKSPACE_STATE_VERSION + 1)).toBe(false);
    expect(isSupportedWorkspaceVersion(0)).toBe(false);
  });
});
