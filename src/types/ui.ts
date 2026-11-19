/** 公共 UI 类型 */

export interface MenuItem {
  key: string;
  label: string;
  /** 快捷键提示，仅展示，不负责绑定 */
  shortcut?: string;
  disabled?: boolean;
  danger?: boolean;
  /** 为 true 时渲染为分隔线，忽略其它字段 */
  divider?: boolean;
  /** 为 true 时在左侧显示勾选标记 */
  checked?: boolean;
}

export type MenuPlacement = 'bottom-start' | 'bottom-end';
