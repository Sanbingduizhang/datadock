<script setup lang="ts">
import {
  Check,
  FolderOpen,
  GitBranch,
  Layers,
  Moon,
  PanelBottom,
  PanelLeft,
  Play,
  Plus,
  RefreshCw,
  Save,
  Settings,
  Square,
  Sun,
  Undo2,
} from 'lucide-vue-next';
import { computed } from 'vue';

import AppDropdown from '@/components/common/AppDropdown.vue';
import AppIconButton from '@/components/common/AppIconButton.vue';

import { useAppStore } from '@/stores/app';
import { useUiStore } from '@/stores/ui';

import type { MenuItem } from '@/types/ui';

/**
 * 顶部工具栏：只放高频操作。
 * 依赖未接入的操作保持禁用并提供提示，避免出现"点了没反应"的按钮。
 */
const props = withDefaults(
  defineProps<{
    /** 当前标签是否可执行 SQL（仅查询标签且已关联连接时为 true） */
    canExecute?: boolean;
    /** 当前标签是否有进行中的事务 */
    inTransaction?: boolean;
    /** 当前工作区名称 */
    workspaceName?: string;
    /** 全部工作区（用于切换） */
    workspaces?: { id: string; name: string }[];
    currentWorkspaceId?: string;
  }>(),
  { canExecute: false, inTransaction: false, workspaceName: '默认工作区', workspaces: () => [], currentWorkspaceId: 'main' },
);

const emit = defineEmits<{
  'create-connection': [];
  'open-sql-file': [];
  'save-sql-file': [];
  execute: [];
  refresh: [];
  'toggle-sidebar': [];
  'toggle-result': [];
  'open-settings': [];
  'begin-transaction': [];
  'commit-transaction': [];
  'rollback-transaction': [];
  'workspace-select': [id: string];
  'workspace-create': [];
  'workspace-rename': [];
  'workspace-delete': [];
}>();

const appStore = useAppStore();
const uiStore = useUiStore();

const workspaceItems = computed<MenuItem[]>(() => {
  const items: MenuItem[] = props.workspaces.map((workspace) => ({
    key: `ws:${workspace.id}`,
    label: workspace.name,
    checked: workspace.id === props.currentWorkspaceId,
  }));

  if (items.length > 0) {
    items.push({ key: 'ws-divider', label: '', divider: true });
  }

  items.push(
    { key: 'ws-create', label: '新建工作区…' },
    { key: 'ws-rename', label: '重命名当前工作区…' },
    { key: 'ws-delete', label: '删除当前工作区' },
  );

  return items;
});

function handleWorkspaceSelect(key: string): void {
  if (key.startsWith('ws:')) {
    emit('workspace-select', key.slice(3));
    return;
  }
  if (key === 'ws-create') {
    emit('workspace-create');
  } else if (key === 'ws-rename') {
    emit('workspace-rename');
  } else if (key === 'ws-delete') {
    emit('workspace-delete');
  }
}

const themeItems = computed<MenuItem[]>(() => [
  { key: 'light', label: '浅色主题', checked: appStore.themeMode === 'light' },
  { key: 'dark', label: '深色主题', checked: appStore.themeMode === 'dark' },
  { key: 'glass', label: '玻璃主题', checked: appStore.themeMode === 'glass' },
  { key: 'skeuomorph', label: '拟物主题', checked: appStore.themeMode === 'skeuomorph' },
  { key: 'system', label: '跟随系统', checked: appStore.themeMode === 'system' },
]);

const ThemeIcon = computed(() => (appStore.monacoTheme === 'dark' ? Moon : Sun));

function handleThemeSelect(key: string): void {
  if (
    key === 'light' ||
    key === 'dark' ||
    key === 'glass' ||
    key === 'skeuomorph' ||
    key === 'system'
  ) {
    appStore.setThemeMode(key);
  }
}
</script>

<template>
  <div class="toolbar">
    <div class="toolbar__group">
      <AppDropdown :items="workspaceItems" placement="bottom-start" @select="handleWorkspaceSelect">
        <template #default="{ toggle }">
          <button type="button" class="workspace" title="切换工作区" @click="toggle">
            <Layers :size="14" />
            <span class="workspace__name">{{ props.workspaceName }}</span>
          </button>
        </template>
      </AppDropdown>

      <span class="toolbar__divider" aria-hidden="true"></span>

      <AppIconButton label="新建连接（⌘N）" @click="emit('create-connection')">
        <Plus :size="17" />
      </AppIconButton>
      <AppIconButton label="打开 SQL 文件（⌘O）" @click="emit('open-sql-file')">
        <FolderOpen :size="17" />
      </AppIconButton>
      <AppIconButton label="保存 SQL（⌘S）" @click="emit('save-sql-file')">
        <Save :size="17" />
      </AppIconButton>
    </div>

    <span class="toolbar__divider" aria-hidden="true"></span>

    <div class="toolbar__group">
      <AppIconButton
        label="执行当前 SQL（⌘/Ctrl + ↵）"
        :disabled="!props.canExecute"
        @click="emit('execute')"
      >
        <Play :size="17" />
      </AppIconButton>
      <AppIconButton label="停止执行（阶段 E 接入）" :disabled="true">
        <Square :size="16" />
      </AppIconButton>
    </div>

    <span class="toolbar__divider" aria-hidden="true"></span>

    <div class="toolbar__group">
      <AppIconButton
        label="开始事务"
        :active="props.inTransaction"
        :disabled="props.inTransaction || !props.canExecute"
        @click="emit('begin-transaction')"
      >
        <GitBranch :size="16" />
      </AppIconButton>
      <AppIconButton
        label="提交事务"
        :disabled="!props.inTransaction"
        @click="emit('commit-transaction')"
      >
        <Check :size="17" />
      </AppIconButton>
      <AppIconButton
        label="回滚事务"
        :disabled="!props.inTransaction"
        @click="emit('rollback-transaction')"
      >
        <Undo2 :size="17" />
      </AppIconButton>
    </div>

    <span class="toolbar__divider" aria-hidden="true"></span>

    <div class="toolbar__group">
      <AppIconButton label="刷新当前视图（⌘R）" @click="emit('refresh')">
        <RefreshCw :size="17" />
      </AppIconButton>
    </div>

    <div class="toolbar__spacer"></div>

    <div class="toolbar__group">
      <AppIconButton
        :label="uiStore.sidebarVisible ? '隐藏导航栏' : '显示导航栏'"
        :active="uiStore.sidebarVisible"
        @click="emit('toggle-sidebar')"
      >
        <PanelLeft :size="17" />
      </AppIconButton>

      <AppIconButton
        :label="uiStore.resultPanelVisible ? '隐藏结果面板' : '显示结果面板'"
        :active="uiStore.resultPanelVisible"
        @click="emit('toggle-result')"
      >
        <PanelBottom :size="17" />
      </AppIconButton>

      <AppDropdown :items="themeItems" placement="bottom-end" @select="handleThemeSelect">
        <template #default="{ toggle }">
          <AppIconButton label="切换主题" @click="toggle">
            <component :is="ThemeIcon" :size="17" />
          </AppIconButton>
        </template>
      </AppDropdown>

      <AppIconButton label="设置（⌘,）" @click="emit('open-settings')">
        <Settings :size="17" />
      </AppIconButton>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--toolbar-height);
  padding: 0 var(--space-2);
  background-color: var(--glass-bg);
  border-bottom: 1px solid var(--border-color);
  backdrop-filter: var(--glass-blur);
}

.workspace {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 160px;
  height: 26px;
  padding: 0 8px;
  color: var(--text-secondary);
  font-size: 11px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.workspace:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.workspace__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.toolbar__group {
  display: flex;
  align-items: center;
  gap: 2px;
}

.toolbar__divider {
  width: 1px;
  height: 18px;
  margin: 0 var(--space-1);
  background-color: var(--divider-color);
}

.toolbar__spacer {
  flex: 1;
}
</style>
