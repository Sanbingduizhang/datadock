<script setup lang="ts">
import { Columns3, Database, FileCode2, House, KeyRound, Table2, X } from 'lucide-vue-next';
import { onBeforeUnmount, ref, watch } from 'vue';

import AppMenu from '@/components/common/AppMenu.vue';

import type { MenuItem } from '@/types/ui';
import type { WorkspaceTab, WorkspaceTabType } from '@/types/workspace';

/**
 * 工作区标签栏。
 * 支持横向滚动、未保存标识、关闭按钮与右键批量关闭。
 */
const props = defineProps<{
  tabs: WorkspaceTab[];
  activeTabId: string | null;
}>();

const emit = defineEmits<{
  activate: [id: string];
  close: [id: string];
  action: [key: string];
}>();

const scrollRef = ref<HTMLElement | null>(null);
const menuVisible = ref(false);
const menuPosition = ref({ x: 0, y: 0 });
const menuItems = ref<MenuItem[]>([]);
const menuTabId = ref<string | null>(null);

const ICONS: Record<WorkspaceTabType, unknown> = {
  welcome: House,
  query: FileCode2,
  table: Table2,
  structure: Columns3,
  'redis-key': KeyRound,
  'redis-db': Database,
};

function iconFor(tab: WorkspaceTab): unknown {
  return ICONS[tab.type];
}

/** 标签过多时支持鼠标滚轮横向滚动 */
function handleWheel(event: WheelEvent): void {
  const element = scrollRef.value;
  if (!element) {
    return;
  }
  if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
    element.scrollLeft += event.deltaY;
  }
}

function handleContextMenu(tab: WorkspaceTab, event: MouseEvent): void {
  menuTabId.value = tab.id;
  menuItems.value = [
    { key: 'close', label: '关闭标签', disabled: !tab.closable },
    { key: 'close-others', label: '关闭其他标签' },
    { key: 'close-right', label: '关闭右侧标签' },
    { key: 'close-all', label: '关闭全部标签' },
  ];
  menuPosition.value = {
    x: Math.min(event.clientX, window.innerWidth - 200),
    y: Math.min(event.clientY, window.innerHeight - 160),
  };
  menuVisible.value = true;
}

function closeMenu(): void {
  menuVisible.value = false;
}

function handleMenuSelect(key: string): void {
  const tabId = menuTabId.value;
  closeMenu();
  if (!tabId) {
    return;
  }

  if (key === 'close') {
    emit('close', tabId);
    return;
  }
  emit('action', `${key}:${tabId}`);
}

function handleDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as HTMLElement;
  if (!target.closest('.tabs-context-menu')) {
    closeMenu();
  }
}

watch(menuVisible, (visible) => {
  if (visible) {
    document.addEventListener('pointerdown', handleDocumentPointerDown);
  } else {
    document.removeEventListener('pointerdown', handleDocumentPointerDown);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown);
});
</script>

<template>
  <div class="tabs">
    <div ref="scrollRef" class="tabs__scroll" @wheel.prevent="handleWheel">
      <button
        v-for="tab in props.tabs"
        :key="tab.id"
        type="button"
        class="tab"
        :class="{ 'is-active': tab.id === props.activeTabId }"
        :title="tab.context.connectionName ? `${tab.title} · ${tab.context.connectionName}` : tab.title"
        @click="emit('activate', tab.id)"
        @auxclick.middle="tab.closable && emit('close', tab.id)"
        @contextmenu.prevent="handleContextMenu(tab, $event)"
      >
        <component :is="iconFor(tab)" :size="13" class="tab__icon" />
        <span class="tab__title">{{ tab.title }}</span>
        <span v-if="tab.dirty" class="tab__dirty" title="未保存"></span>
        <span
          v-else-if="tab.closable"
          class="tab__close"
          role="button"
          aria-label="关闭标签"
          @click.stop="emit('close', tab.id)"
        >
          <X :size="12" />
        </span>
      </button>
    </div>

    <Teleport to="body">
      <div
        v-if="menuVisible"
        class="tabs-context-menu"
        :style="{ left: `${menuPosition.x}px`, top: `${menuPosition.y}px` }"
      >
        <AppMenu :items="menuItems" :min-width="168" @select="handleMenuSelect" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.tabs {
  display: flex;
  flex: none;
  align-items: stretch;
  height: var(--tabbar-height);
  background-color: var(--surface-subtle);
  border-bottom: 1px solid var(--border-color);
}

.tabs__scroll {
  display: flex;
  flex: 1;
  align-items: stretch;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}

.tabs__scroll::-webkit-scrollbar {
  height: 0;
}

.tab {
  display: flex;
  flex: none;
  align-items: center;
  gap: 6px;
  max-width: 200px;
  padding: 0 var(--space-2) 0 var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  border-right: 1px solid var(--divider-color);
  border-bottom: 1px solid transparent;
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast);
}

.tab:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.tab.is-active {
  color: var(--text-primary);
  background-color: var(--surface-bg);
  border-bottom-color: var(--primary-color);
}

.tab__icon {
  flex: none;
  opacity: 0.75;
}

.tab__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab__dirty {
  flex: none;
  width: 6px;
  height: 6px;
  background-color: var(--primary-color);
  border-radius: 50%;
}

.tab__close {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  margin-left: 2px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
  opacity: 0;
  transition:
    opacity var(--transition-fast),
    background-color var(--transition-fast);
}

.tab:hover .tab__close,
.tab.is-active .tab__close {
  opacity: 1;
}

.tab__close:hover {
  color: var(--text-primary);
  background-color: var(--surface-active);
}

.tabs-context-menu {
  position: fixed;
  z-index: var(--z-dropdown);
}
</style>
