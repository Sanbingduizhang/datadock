import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';

import { LAYOUT } from '@/constants/layout';
import { useUiStore } from '@/stores/ui';

describe('ui store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('初始尺寸使用布局常量', () => {
    const store = useUiStore();

    expect(store.sidebarWidth).toBe(LAYOUT.sidebar.defaultWidth);
    expect(store.resultPanelHeight).toBe(LAYOUT.resultPanel.defaultHeight);
    expect(store.sidebarVisible).toBe(true);
    expect(store.resultPanelVisible).toBe(true);
  });

  it('导航栏宽度受上下限约束', () => {
    const store = useUiStore();

    store.setSidebarWidth(80);
    expect(store.sidebarWidth).toBe(LAYOUT.sidebar.minWidth);

    store.setSidebarWidth(900);
    expect(store.sidebarWidth).toBe(LAYOUT.sidebar.maxWidth);
  });

  it('结果面板高度不低于最小值', () => {
    const store = useUiStore();

    store.setResultPanelHeight(20);
    expect(store.resultPanelHeight).toBe(LAYOUT.resultPanel.minHeight);
  });

  it('结果面板高度受工作区高度比例限制', () => {
    const store = useUiStore();
    const workspaceHeight = 600;

    store.setResultPanelHeight(1000, workspaceHeight);

    expect(store.resultPanelHeight).toBe(
      Math.floor(workspaceHeight * LAYOUT.resultPanel.maxHeightRatio),
    );
  });

  it('面板可见性可以切换', () => {
    const store = useUiStore();

    store.toggleSidebar();
    expect(store.sidebarVisible).toBe(false);

    store.toggleResultPanel();
    expect(store.resultPanelVisible).toBe(false);

    store.toggleStatusBar();
    expect(store.statusBarVisible).toBe(false);
  });
});
