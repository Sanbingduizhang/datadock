/** 主界面布局尺寸（对应 docs/02 2.2 建议值） */
export const LAYOUT = {
  menuBarHeight: 32,
  toolbarHeight: 44,
  tabBarHeight: 36,
  statusBarHeight: 24,
  sidebar: {
    defaultWidth: 250,
    minWidth: 180,
    maxWidth: 420,
  },
  resultPanel: {
    defaultHeight: 220,
    minHeight: 100,
    /** 结果面板最多占工作区高度的比例，避免挤占编辑器 */
    maxHeightRatio: 0.7,
  },
} as const;
