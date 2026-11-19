import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import type { WorkspaceTab, WorkspaceTabContext, WorkspaceTabType } from '@/types/workspace';

type TableContext = WorkspaceTabContext & { databaseName: string; tableName: string };
type RedisKeyContext = WorkspaceTabContext & { keyName: string };
type RedisDbContext = WorkspaceTabContext & { databaseName: string };

let tabSequence = 0;
let querySequence = 0;

function nextTabId(): string {
  tabSequence += 1;
  return `tab-${tabSequence}`;
}

/**
 * 工作区标签管理。
 * 只维护标签本身的状态与上下文，不负责 SQL 执行、数据查询等业务。
 */
export const useWorkspaceStore = defineStore('workspace', () => {
  const tabs = ref<WorkspaceTab[]>([]);
  const activeTabId = ref<string | null>(null);

  const activeTab = computed<WorkspaceTab | null>(
    () => tabs.value.find((tab) => tab.id === activeTabId.value) ?? null,
  );
  const tabCount = computed(() => tabs.value.length);
  const hasDirtyTab = computed(() => tabs.value.some((tab) => tab.dirty));

  function activateTab(tabId: string): void {
    if (tabs.value.some((tab) => tab.id === tabId)) {
      activeTabId.value = tabId;
    }
  }

  /** 打开欢迎页；已存在时直接激活，不重复创建 */
  function openWelcomeTab(): WorkspaceTab {
    const existing = tabs.value.find((tab) => tab.type === 'welcome');
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }

    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'welcome',
      title: '欢迎',
      context: {},
      closable: false,
      dirty: false,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 新建 SQL 查询标签，每次都是独立标签 */
  function openQueryTab(context: WorkspaceTabContext = {}): WorkspaceTab {
    querySequence += 1;
    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'query',
      title: `SQL 查询 ${querySequence}`,
      context: { ...context },
      closable: true,
      dirty: false,
      draft: '',
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  function isSameContext(left: WorkspaceTabContext, right: WorkspaceTabContext): boolean {
    return (
      left.connectionId === right.connectionId &&
      left.databaseName === right.databaseName &&
      left.tableName === right.tableName
    );
  }

  /** 打开表数据标签：同一连接、同一数据库、同一张表复用已有标签 */
  function openTableTab(context: TableContext): WorkspaceTab {
    const existing = tabs.value.find(
      (tab) => tab.type === 'table' && isSameContext(tab.context, context),
    );
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }

    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'table',
      title: context.tableName,
      context: { ...context },
      closable: true,
      dirty: false,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 打开表结构标签，复用规则同表数据 */
  function openStructureTab(context: TableContext): WorkspaceTab {
    const existing = tabs.value.find(
      (tab) => tab.type === 'structure' && isSameContext(tab.context, context),
    );
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }

    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'structure',
      title: `${context.tableName} - 结构`,
      context: { ...context },
      closable: true,
      dirty: false,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 打开 Redis 键值查看器标签：同一连接、同一键名复用已有标签 */
  function openRedisKeyTab(context: RedisKeyContext): WorkspaceTab {
    const existing = tabs.value.find(
      (tab) => tab.type === 'redis-key' && tab.context.keyName === context.keyName,
    );
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }

    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'redis-key',
      title: context.keyName,
      context: { ...context },
      closable: true,
      dirty: false,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 打开 Redis 库浏览标签：同一连接、同一库复用已有标签（右侧主区域浏览） */
  function openRedisDbTab(context: RedisDbContext): WorkspaceTab {
    const existing = tabs.value.find(
      (tab) =>
        tab.type === 'redis-db' &&
        tab.context.connectionId === context.connectionId &&
        tab.context.databaseName === context.databaseName,
    );
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }

    const tab: WorkspaceTab = {
      id: nextTabId(),
      type: 'redis-db',
      title: `db${context.redisDbIndex ?? 0}`,
      context: { ...context },
      closable: true,
      dirty: false,
    };
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 更新查询标签的编辑内容，并标记未保存状态 */
  function updateDraft(tabId: string, content: string): void {
    const tab = tabs.value.find((item) => item.id === tabId);
    if (!tab || tab.type !== 'query') {
      return;
    }
    tab.draft = content;
    tab.dirty = content.length > 0;
  }

  /** 设置标签标题（打开/保存 SQL 文件后使用文件名） */
  function setTabTitle(tabId: string, title: string): void {
    const tab = tabs.value.find((item) => item.id === tabId);
    const trimmed = title.trim();
    if (!tab || trimmed.length === 0) {
      return;
    }
    tab.title = trimmed.slice(0, 200);
  }

  /** 取消未保存标记（阶段 E 保存 SQL 文件后调用） */
  function markClean(tabId: string): void {
    const tab = tabs.value.find((item) => item.id === tabId);
    if (tab) {
      tab.dirty = false;
    }
  }

  /** 关闭标签，返回是否实际关闭 */
  function closeTab(tabId: string): boolean {
    const index = tabs.value.findIndex((tab) => tab.id === tabId);
    if (index < 0) {
      return false;
    }

    const tab = tabs.value[index];
    if (!tab || !tab.closable) {
      return false;
    }

    tabs.value.splice(index, 1);
    if (activeTabId.value === tabId) {
      const next = tabs.value[index] ?? tabs.value[index - 1] ?? null;
      activeTabId.value = next ? next.id : null;
    }

    if (tabs.value.length === 0) {
      openWelcomeTab();
    }

    return true;
  }

  function closeOtherTabs(tabId: string): void {
    tabs.value = tabs.value.filter((tab) => tab.id === tabId || !tab.closable);
    if (!tabs.value.some((tab) => tab.id === activeTabId.value)) {
      activeTabId.value = tabId;
    }
  }

  function closeTabsToRight(tabId: string): void {
    const index = tabs.value.findIndex((tab) => tab.id === tabId);
    if (index < 0) {
      return;
    }

    tabs.value = tabs.value.filter((tab, currentIndex) => currentIndex <= index || !tab.closable);
    if (!tabs.value.some((tab) => tab.id === activeTabId.value)) {
      activeTabId.value = tabId;
    }
  }

  function closeAllTabs(): void {
    tabs.value = tabs.value.filter((tab) => !tab.closable);
    activeTabId.value = tabs.value.at(-1)?.id ?? null;
    if (tabs.value.length === 0) {
      openWelcomeTab();
    }
  }

  /** 判断某标签是否存在未保存内容，供关闭前确认使用 */
  function isDirty(tabId: string): boolean {
    return tabs.value.find((tab) => tab.id === tabId)?.dirty ?? false;
  }

  /**
   * 用持久化状态恢复标签。
   * 过滤器重复一次类型校验，防止旧版本或被手工修改的数据造成异常标签。
   */
  function restoreTabs(restored: WorkspaceTab[], restoredActiveId?: string): void {
    const allowed: WorkspaceTabType[] = [
      'welcome',
      'query',
      'table',
      'structure',
      'redis-key',
      'redis-db',
    ];
    const valid = restored.filter(
      (tab) => tab.id.length > 0 && allowed.includes(tab.type) && tab.title.length > 0,
    );

    if (valid.length === 0) {
      reset();
      openWelcomeTab();
      return;
    }

    tabs.value = valid;
    // 恢复后新建查询的编号不能与已有标签重复
    querySequence = valid.filter((tab) => tab.type === 'query').length;
    activeTabId.value =
      restoredActiveId && valid.some((tab) => tab.id === restoredActiveId)
        ? restoredActiveId
        : (valid[0]?.id ?? null);
  }

  function reset(): void {
    tabs.value = [];
    activeTabId.value = null;
  }

  /** 按类型统计标签数量（用于状态栏） */
  function countByType(type: WorkspaceTabType): number {
    return tabs.value.filter((tab) => tab.type === type).length;
  }

  return {
    tabs,
    activeTabId,
    activeTab,
    tabCount,
    hasDirtyTab,
    activateTab,
    openWelcomeTab,
    openQueryTab,
    openTableTab,
    openStructureTab,
    openRedisKeyTab,
    openRedisDbTab,
    updateDraft,
    setTabTitle,
    markClean,
    closeTab,
    closeOtherTabs,
    closeTabsToRight,
    closeAllTabs,
    isDirty,
    restoreTabs,
    countByType,
    reset,
  };
});
