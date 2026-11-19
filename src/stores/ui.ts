import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import { LAYOUT } from '@/constants/layout';

/**
 * 全局 UI 状态：面板可见性与尺寸。
 * 面板尺寸由用户拖拽产生，统一在此做边界收敛，避免组件各自校验。
 * 阶段 C 接入本地 SQLite 后，此处状态将随工作区状态持久化。
 */
export const useUiStore = defineStore('ui', () => {
  const sidebarVisible = ref(true);
  const sidebarWidth = ref<number>(LAYOUT.sidebar.defaultWidth);

  const resultPanelVisible = ref(true);
  const resultPanelHeight = ref<number>(LAYOUT.resultPanel.defaultHeight);

  const statusBarVisible = ref(true);

  /** 底部 SQL 日志面板（会话内展示执行过的语句） */
  const sqlLogVisible = ref(false);

  const canCollapseResultPanel = computed(() => resultPanelVisible.value);

  function clampSidebarWidth(width: number): number {
    return Math.min(Math.max(width, LAYOUT.sidebar.minWidth), LAYOUT.sidebar.maxWidth);
  }

  function setSidebarWidth(width: number): void {
    sidebarWidth.value = clampSidebarWidth(width);
  }

  function toggleSidebar(): void {
    sidebarVisible.value = !sidebarVisible.value;
  }

  function setSidebarVisible(visible: boolean): void {
    sidebarVisible.value = visible;
  }

  /** 结果面板高度需要同时受最小值与工作区高度限制 */
  function setResultPanelHeight(height: number, workspaceHeight?: number): void {
    const maxHeight = workspaceHeight
      ? Math.max(
          LAYOUT.resultPanel.minHeight,
          Math.floor(workspaceHeight * LAYOUT.resultPanel.maxHeightRatio),
        )
      : Number.POSITIVE_INFINITY;
    resultPanelHeight.value = Math.min(
      Math.max(height, LAYOUT.resultPanel.minHeight),
      maxHeight,
    );
  }

  function toggleResultPanel(): void {
    resultPanelVisible.value = !resultPanelVisible.value;
  }

  function setResultPanelVisible(visible: boolean): void {
    resultPanelVisible.value = visible;
  }

  function toggleSqlLog(): void {
    sqlLogVisible.value = !sqlLogVisible.value;
  }

  function setSqlLogVisible(visible: boolean): void {
    sqlLogVisible.value = visible;
  }

  function toggleStatusBar(): void {
    statusBarVisible.value = !statusBarVisible.value;
  }

  function setStatusBarVisible(visible: boolean): void {
    statusBarVisible.value = visible;
  }

  // 侧栏面板的数据版本号：执行/收藏后自增，面板据此重新拉取
  const historyVersion = ref(0);
  const favoritesVersion = ref(0);

  function bumpHistory(): void {
    historyVersion.value += 1;
  }

  function bumpFavorites(): void {
    favoritesVersion.value += 1;
  }

  return {
    sidebarVisible,
    sqlLogVisible,
    toggleSqlLog,
    setSqlLogVisible,
    historyVersion,
    favoritesVersion,
    bumpHistory,
    bumpFavorites,
    sidebarWidth,
    resultPanelVisible,
    resultPanelHeight,
    statusBarVisible,
    canCollapseResultPanel,
    clampSidebarWidth,
    setSidebarWidth,
    toggleSidebar,
    setSidebarVisible,
    setResultPanelHeight,
    toggleResultPanel,
    setResultPanelVisible,
    toggleStatusBar,
    setStatusBarVisible,
  };
});
