<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';

import AppMenu from '@/components/common/AppMenu.vue';
import AppPopover from '@/components/common/AppPopover.vue';

import { useAppStore } from '@/stores/app';
import { useUiStore } from '@/stores/ui';

import type { MenuItem } from '@/types/ui';

/**
 * 顶部菜单栏。
 * 菜单项按 docs/02 第 3 章定义；未接入功能统一置灰，不伪造可用状态。
 * 实际动作通过 action 事件交给工作区统一调度。
 */
const emit = defineEmits<{ action: [key: string] }>();

const appStore = useAppStore();
const uiStore = useUiStore();

const openIndex = ref<number | null>(null);
const menuBarRef = ref<HTMLElement | null>(null);
/** 每个菜单标题的 DOM，用于给浮层定位 */
const titleRefs = ref<(HTMLElement | null)[]>([]);
/** 每个浮层实例，用于判断「点击是否落在面板内」 */
const popoverRefs = ref<(InstanceType<typeof AppPopover> | null)[]>([]);

function setTitleRef(el: Element | null, index: number): void {
  titleRefs.value[index] = (el as HTMLElement | null) ?? null;
}

function setPopoverRef(el: InstanceType<typeof AppPopover> | null, index: number): void {
  popoverRefs.value[index] = el;
}

const isMac = navigator.userAgent.includes('Mac');
const MOD = isMac ? '⌘' : 'Ctrl+';

const themeMode = computed(() => appStore.resolvedTheme);
const themeLabel = computed(() => (themeMode.value === 'dark' ? '深色主题' : '浅色主题'));

interface MenuGroup {
  key: string;
  label: string;
  items: MenuItem[];
}

const menus = computed<MenuGroup[]>(() => [
  {
    key: 'file',
    label: '文件',
    items: [
      { key: 'file.newConnection', label: '新建连接', shortcut: `${MOD}N` },
      { key: 'file.openSql', label: '打开 SQL 文件…', shortcut: `${MOD}O` },
      { key: 'file.saveSql', label: '保存 SQL', shortcut: `${MOD}S` },
      { key: 'file.divider', label: '', divider: true },
      { key: 'file.exit', label: '退出 DataDock' },
    ],
  },
  {
    key: 'edit',
    label: '编辑',
    items: [
      { key: 'edit.undo', label: '撤销', shortcut: `${MOD}Z`, disabled: true },
      { key: 'edit.redo', label: '重做', shortcut: `${MOD}⇧Z`, disabled: true },
      { key: 'edit.divider', label: '', divider: true },
      { key: 'edit.cut', label: '剪切', shortcut: `${MOD}X`, disabled: true },
      { key: 'edit.copy', label: '复制', shortcut: `${MOD}C`, disabled: true },
      { key: 'edit.paste', label: '粘贴', shortcut: `${MOD}V`, disabled: true },
      { key: 'edit.divider2', label: '', divider: true },
      { key: 'edit.find', label: '查找', shortcut: `${MOD}F`, disabled: true },
      { key: 'edit.replace', label: '替换', shortcut: `${MOD}H`, disabled: true },
    ],
  },
  {
    key: 'view',
    label: '查看',
    items: [
      {
        key: 'view.sidebar',
        label: '显示导航栏',
        checked: uiStore.sidebarVisible,
      },
      {
        key: 'view.resultPanel',
        label: '显示结果面板',
        checked: uiStore.resultPanelVisible,
      },
      {
        key: 'view.statusBar',
        label: '显示状态栏',
        checked: uiStore.statusBarVisible,
      },
      {
        key: 'view.sqlLog',
        label: '显示 SQL 日志',
        checked: uiStore.sqlLogVisible,
      },
      { key: 'view.divider', label: '', divider: true },
      {
        key: 'view.themeLight',
        label: '浅色主题',
        checked: appStore.themeMode === 'light',
      },
      {
        key: 'view.themeDark',
        label: '深色主题',
        checked: appStore.themeMode === 'dark',
      },
      {
        key: 'view.themeGlass',
        label: '玻璃主题',
        checked: appStore.themeMode === 'glass',
      },
      {
        key: 'view.themeSkeuomorph',
        label: '拟物主题',
        checked: appStore.themeMode === 'skeuomorph',
      },
      {
        key: 'view.themeSystem',
        label: '跟随系统',
        checked: appStore.themeMode === 'system',
      },
    ],
  },
  {
    key: 'database',
    label: '数据库',
    items: [
      { key: 'database.connect', label: '连接', disabled: true },
      { key: 'database.disconnect', label: '断开连接', disabled: true },
      { key: 'database.refresh', label: '刷新对象树', disabled: true },
      { key: 'database.selectDatabase', label: '选择数据库', disabled: true },
      { key: 'database.divider', label: '', divider: true },
      { key: 'database.export', label: '导出数据库…' },
      { key: 'database.import', label: '导入数据库…' },
    ],
  },
  {
    key: 'sql',
    label: 'SQL',
    items: [
      { key: 'sql.executeCurrent', label: '执行当前语句', shortcut: `${MOD}↵`, disabled: true },
      { key: 'sql.executeAll', label: '执行全部', shortcut: `${MOD}⇧↵`, disabled: true },
      { key: 'sql.stop', label: '停止执行', disabled: true },
      { key: 'sql.divider', label: '', divider: true },
      { key: 'sql.format', label: '格式化 SQL', disabled: true },
    ],
  },
  {
    key: 'tools',
    label: '工具',
    items: [
      { key: 'tools.history', label: '查询历史', disabled: true },
      { key: 'tools.favorites', label: 'SQL 收藏', disabled: true },
      { key: 'tools.divider', label: '', divider: true },
      { key: 'tools.settings', label: '设置', shortcut: `${MOD},` },
    ],
  },
  {
    key: 'window',
    label: '窗口',
    items: [
      { key: 'window.closeTab', label: '关闭当前标签', shortcut: `${MOD}W` },
      { key: 'window.closeOtherTabs', label: '关闭其他标签' },
      { key: 'window.closeAllTabs', label: '关闭全部标签' },
    ],
  },
  {
    key: 'help',
    label: '帮助',
    items: [{ key: 'help.about', label: '关于 DataDock' }],
  },
]);

function toggleMenu(index: number): void {
  openIndex.value = openIndex.value === index ? null : index;
}

/** 已展开菜单时，鼠标滑过其他菜单标题直接切换（桌面应用习惯） */
function hoverMenu(index: number): void {
  if (openIndex.value !== null && openIndex.value !== index) {
    openIndex.value = index;
  }
}

function closeMenu(): void {
  openIndex.value = null;
}

function handleSelect(key: string): void {
  closeMenu();
  emit('action', key);
}

function handleDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as Node | null;
  if (menuBarRef.value?.contains(target)) {
    return;
  }
  // 面板已 Teleport 到 body，需要单独判断
  if (popoverRefs.value.some((item) => item?.contains(target))) {
    return;
  }
  closeMenu();
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    closeMenu();
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', handleDocumentPointerDown);
  document.addEventListener('keydown', handleDocumentKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown);
  document.removeEventListener('keydown', handleDocumentKeydown);
});
</script>

<template>
  <header ref="menuBarRef" class="menubar">
    <div class="menubar__brand">
      <span class="brand-mark" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="brand-name">DataDock</span>
    </div>

    <nav class="menubar__nav">
      <div v-for="(menu, index) in menus" :key="menu.key" class="menubar__group">
        <button
          :ref="(el) => setTitleRef(el as Element | null, index)"
          type="button"
          class="menubar__title"
          :class="{ 'is-open': openIndex === index }"
          @click="toggleMenu(index)"
          @mouseenter="hoverMenu(index)"
        >
          {{ menu.label }}
        </button>

        <AppPopover
          :ref="(el) => setPopoverRef(el as InstanceType<typeof AppPopover> | null, index)"
          :open="openIndex === index"
          :anchor="titleRefs[index] ?? null"
          :min-width="200"
        >
          <AppMenu :items="menu.items" @select="handleSelect" />
        </AppPopover>
      </div>
    </nav>

    <div class="menubar__spacer"></div>

    <span class="menubar__state">{{ themeLabel }}</span>
  </header>
</template>

<style scoped>
.menubar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--titlebar-height);
  padding: 0 var(--space-2);
  background-color: var(--glass-bg);
  border-bottom: 1px solid var(--border-color);
  backdrop-filter: var(--glass-blur);
}

.menubar__brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-left: var(--space-1);
}

.brand-mark {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 14px;
}

.brand-mark i {
  display: block;
  width: 3px;
  background-color: var(--primary-color);
  border-radius: 1px;
}

.brand-mark i:nth-child(1) {
  height: 6px;
}

.brand-mark i:nth-child(2) {
  height: 11px;
}

.brand-mark i:nth-child(3) {
  height: 8px;
}

.brand-name {
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 600;
  letter-spacing: 0.2px;
}

.menubar__nav {
  display: flex;
  align-items: center;
  gap: 1px;
  margin-left: var(--space-2);
}

.menubar__group {
  position: relative;
}

.menubar__title {
  height: 24px;
  padding: 0 9px;
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  border-radius: var(--radius-sm);
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast);
}

.menubar__title:hover,
.menubar__title.is-open {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.menubar__spacer {
  flex: 1;
}

.menubar__state {
  padding-right: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

</style>
