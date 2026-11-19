import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';

import { useWorkspaceStore } from '@/stores/workspace';

const TABLE_CONTEXT = {
  connectionId: 'conn-dev',
  connectionName: 'MySQL-开发环境',
  databaseName: 'database_a',
  tableName: 'users',
};

describe('workspace store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('欢迎页只创建一次', () => {
    const store = useWorkspaceStore();

    const first = store.openWelcomeTab();
    const second = store.openWelcomeTab();

    expect(store.tabs).toHaveLength(1);
    expect(first.id).toBe(second.id);
    expect(store.activeTabId).toBe(first.id);
    expect(first.closable).toBe(false);
  });

  it('每次新建查询生成独立标签并递增标题', () => {
    const store = useWorkspaceStore();

    const first = store.openQueryTab();
    const second = store.openQueryTab();

    expect(first.id).not.toBe(second.id);
    expect(first.title).toBe('SQL 查询 1');
    expect(second.title).toBe('SQL 查询 2');
    expect(store.activeTabId).toBe(second.id);
  });

  it('同一张表重复打开时复用标签', () => {
    const store = useWorkspaceStore();

    const first = store.openTableTab(TABLE_CONTEXT);
    const second = store.openTableTab({ ...TABLE_CONTEXT });

    expect(store.tabs).toHaveLength(1);
    expect(second.id).toBe(first.id);
  });

  it('不同数据库的同名表不会错误复用', () => {
    const store = useWorkspaceStore();

    store.openTableTab(TABLE_CONTEXT);
    store.openTableTab({ ...TABLE_CONTEXT, databaseName: 'database_b' });

    expect(store.tabs).toHaveLength(2);
  });

  it('表数据与表结构是两类独立标签', () => {
    const store = useWorkspaceStore();

    store.openTableTab(TABLE_CONTEXT);
    store.openStructureTab(TABLE_CONTEXT);

    expect(store.tabs.map((tab) => tab.type)).toEqual(['table', 'structure']);
    expect(store.tabs[1]?.title).toBe('users - 结构');
  });

  it('编辑内容会标记未保存状态', () => {
    const store = useWorkspaceStore();
    const tab = store.openQueryTab();

    store.updateDraft(tab.id, 'SELECT 1;');
    expect(store.tabs[0]?.dirty).toBe(true);
    expect(store.isDirty(tab.id)).toBe(true);

    store.markClean(tab.id);
    expect(store.isDirty(tab.id)).toBe(false);
  });

  it('关闭当前标签后激活相邻标签', () => {
    const store = useWorkspaceStore();
    const first = store.openQueryTab();
    const second = store.openQueryTab();

    expect(store.activeTabId).toBe(second.id);

    store.closeTab(second.id);
    expect(store.activeTabId).toBe(first.id);
  });

  it('不可关闭的标签不会被关闭', () => {
    const store = useWorkspaceStore();
    const welcome = store.openWelcomeTab();

    expect(store.closeTab(welcome.id)).toBe(false);
    expect(store.tabs).toHaveLength(1);
  });

  it('关闭全部标签后回到欢迎页', () => {
    const store = useWorkspaceStore();
    store.openQueryTab();
    store.openQueryTab();

    store.closeAllTabs();

    expect(store.tabs).toHaveLength(1);
    expect(store.tabs[0]?.type).toBe('welcome');
    expect(store.activeTabId).toBe(store.tabs[0]?.id);
  });

  it('关闭其他标签时保留不可关闭的标签', () => {
    const store = useWorkspaceStore();
    const welcome = store.openWelcomeTab();
    const first = store.openQueryTab();
    const second = store.openQueryTab();

    store.closeOtherTabs(second.id);

    expect(store.tabs.map((tab) => tab.id)).toEqual([welcome.id, second.id]);
    expect(store.tabs.some((tab) => tab.id === first.id)).toBe(false);
  });

  it('关闭右侧标签只影响右侧的可关闭标签', () => {
    const store = useWorkspaceStore();
    const first = store.openQueryTab();
    const second = store.openQueryTab();
    store.openQueryTab();

    store.closeTabsToRight(first.id);

    expect(store.tabs).toHaveLength(1);
    expect(store.tabs[0]?.id).toBe(first.id);
    expect(store.activeTabId).toBe(first.id);
    expect(store.tabs.some((tab) => tab.id === second.id)).toBe(false);
  });
});
