import { onBeforeUnmount, watch } from 'vue';

import { getWorkspace, saveWorkspace } from '@/api/workspace';
import { useToast } from '@/composables/useToast';
import { useUiStore } from '@/stores/ui';
import { useWorkspaceStore } from '@/stores/workspace';
import { useWorkspacesStore } from '@/stores/workspaces';
import { normalizeError } from '@/utils/errors';
import {
  buildPersistedState,
  isSupportedWorkspaceVersion,
  toWorkspaceTabs,
} from '@/utils/workspace-persist';

import type { PersistedWorkspaceState } from '@/types/workspace';

/** 布局/标签变化后的保存延迟，避免拖拽面板时写入过于频繁 */
const SAVE_DEBOUNCE_MS = 400;

/**
 * 工作区持久化：启动时恢复布局与标签，运行中变更自动保存到本地 SQLite。
 * 只保存布局与标签元数据（含 SQL 草稿），不保存任何连接凭据。
 */
export function useWorkspacePersistence() {
  const uiStore = useUiStore();
  const workspaceStore = useWorkspaceStore();
  const workspaces = useWorkspacesStore();
  const toast = useToast();

  let timer: ReturnType<typeof setTimeout> | undefined;
  let enabled = false;

  function buildState(): PersistedWorkspaceState {
    return buildPersistedState({
      layout: {
        sidebarWidth: uiStore.sidebarWidth,
        sidebarVisible: uiStore.sidebarVisible,
        resultPanelHeight: uiStore.resultPanelHeight,
        resultPanelVisible: uiStore.resultPanelVisible,
        statusBarVisible: uiStore.statusBarVisible,
      },
      tabs: workspaceStore.tabs,
      activeTabId: workspaceStore.activeTabId,
      name: workspaces.currentName,
    });
  }

  async function flush(): Promise<void> {
    try {
      await saveWorkspace(buildState(), workspaces.currentId);
    } catch (error) {
      toast.error(`工作区保存失败：${normalizeError(error).message}`);
    }
  }

  function scheduleSave(): void {
    if (!enabled) {
      return;
    }
    clearTimeout(timer);
    timer = setTimeout(() => {
      void flush();
    }, SAVE_DEBOUNCE_MS);
  }

  /** 恢复上次的工作区；返回是否成功恢复 */
  async function restore(): Promise<boolean> {
    try {
      const response = await getWorkspace(workspaces.currentId);
      const state = response.state;

      if (!state) {
        return false;
      }

      if (!isSupportedWorkspaceVersion(state.version)) {
        toast.warning('工作区状态版本不兼容，已使用默认布局');
        return false;
      }

      uiStore.setSidebarWidth(state.layout.sidebarWidth);
      uiStore.setSidebarVisible(state.layout.sidebarVisible);
      uiStore.setResultPanelHeight(state.layout.resultPanelHeight);
      uiStore.setResultPanelVisible(state.layout.resultPanelVisible);
      uiStore.setStatusBarVisible(state.layout.statusBarVisible);

      if (state.name) {
        workspaces.renameCurrent(state.name);
      }

      workspaceStore.restoreTabs(toWorkspaceTabs(state), state.activeTabId);
      return true;
    } catch (error) {
      toast.error(`工作区恢复失败：${normalizeError(error).message}`);
      return false;
    }
  }

  function handlePageHide(): void {
    if (enabled) {
      void flush();
    }
  }

  /** 开始监听变更并自动保存 */
  function enable(): void {
    if (enabled) {
      return;
    }
    enabled = true;

    watch(
      () => ({
        sidebarWidth: uiStore.sidebarWidth,
        sidebarVisible: uiStore.sidebarVisible,
        resultPanelHeight: uiStore.resultPanelHeight,
        resultPanelVisible: uiStore.resultPanelVisible,
        statusBarVisible: uiStore.statusBarVisible,
        activeTabId: workspaceStore.activeTabId,
        tabs: workspaceStore.tabs,
      }),
      scheduleSave,
      { deep: true },
    );

    window.addEventListener('pagehide', handlePageHide);
  }

  onBeforeUnmount(() => {
    clearTimeout(timer);
    window.removeEventListener('pagehide', handlePageHide);
    if (enabled) {
      void flush();
    }
  });

  return { restore, enable, flushNow: flush };
}
