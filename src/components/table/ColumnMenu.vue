<script setup lang="ts">
import { ArrowDown, ArrowUp, Columns3, Eye, EyeOff } from 'lucide-vue-next';
import { onBeforeUnmount, ref, watch } from 'vue';

import type { QueryColumn } from '@/types/query';

/**
 * 列显示/顺序菜单：勾选控制显示，上下箭头调整顺序。
 * 顺序与显隐只影响当前表标签，不写入数据库。
 */
const props = defineProps<{
  columns: QueryColumn[];
  hidden: string[];
}>();

const emit = defineEmits<{
  toggle: [name: string];
  move: [name: string, direction: -1 | 1];
  reset: [];
}>();

const rootRef = ref<HTMLElement | null>(null);
const open = ref(false);

function togglePanel(): void {
  open.value = !open.value;
}

function isHidden(name: string): boolean {
  return props.hidden.includes(name);
}

function handleDocumentPointerDown(event: PointerEvent): void {
  if (!rootRef.value?.contains(event.target as Node)) {
    open.value = false;
  }
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    open.value = false;
  }
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener('pointerdown', handleDocumentPointerDown);
    document.addEventListener('keydown', handleDocumentKeydown);
  } else {
    document.removeEventListener('pointerdown', handleDocumentPointerDown);
    document.removeEventListener('keydown', handleDocumentKeydown);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown);
  document.removeEventListener('keydown', handleDocumentKeydown);
});
</script>

<template>
  <div ref="rootRef" class="column-menu">
    <button
      type="button"
      class="column-menu__trigger"
      :class="{ 'is-active': hidden.length > 0 }"
      title="显示/隐藏列、调整顺序"
      @click="togglePanel"
    >
      <Columns3 :size="13" />
      列
      <span v-if="hidden.length > 0" class="column-menu__count">
        {{ columns.length - hidden.length }}/{{ columns.length }}
      </span>
    </button>

    <Transition name="fade">
      <div v-if="open" class="column-menu__panel">
        <header class="column-menu__head">
          <span>列的显示与顺序</span>
          <button type="button" class="column-menu__link" @click="emit('reset')">恢复默认</button>
        </header>

        <ul class="column-menu__list">
          <li v-for="(column, index) in columns" :key="column.name" class="column-menu__item">
            <button
              type="button"
              class="column-menu__toggle"
              :title="isHidden(column.name) ? '显示该列' : '隐藏该列'"
              @click="emit('toggle', column.name)"
            >
              <EyeOff v-if="isHidden(column.name)" :size="12" />
              <Eye v-else :size="12" />
              <span :class="{ 'is-hidden': isHidden(column.name) }">{{ column.name }}</span>
            </button>

            <span class="column-menu__spacer"></span>

            <button
              type="button"
              class="column-menu__move"
              :disabled="index === 0"
              title="上移"
              @click="emit('move', column.name, -1)"
            >
              <ArrowUp :size="11" />
            </button>
            <button
              type="button"
              class="column-menu__move"
              :disabled="index === columns.length - 1"
              title="下移"
              @click="emit('move', column.name, 1)"
            >
              <ArrowDown :size="11" />
            </button>
          </li>
        </ul>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.column-menu {
  position: relative;
  display: inline-flex;
}

.column-menu__trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 8px;
  color: var(--text-secondary);
  font-size: 11px;
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.column-menu__trigger:hover {
  color: var(--text-primary);
}

.column-menu__trigger.is-active {
  color: var(--primary-color);
  border-color: var(--primary-color);
}

.column-menu__count {
  color: var(--text-tertiary);
  font-size: 10px;
}

.column-menu__panel {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: var(--z-dropdown);
  width: 240px;
  max-height: 320px;
  overflow: auto;
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
}

.column-menu__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  color: var(--text-tertiary);
  font-size: 10px;
  border-bottom: 1px solid var(--divider-color);
}

.column-menu__link {
  margin-left: auto;
  color: var(--primary-color);
  font-size: 10px;
}

.column-menu__item {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px var(--space-2);
}

.column-menu__item:hover {
  background-color: var(--surface-hover);
}

.column-menu__toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 6px;
  min-width: 0;
  padding: 2px 4px;
  color: var(--text-secondary);
  font-size: 11px;
  text-align: left;
}

.column-menu__toggle span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.column-menu__toggle span.is-hidden {
  color: var(--text-tertiary);
  text-decoration: line-through;
}

.column-menu__spacer {
  flex: 1;
}

.column-menu__move {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
}

.column-menu__move:hover:not(:disabled) {
  color: var(--text-primary);
  background-color: var(--surface-active);
}

.column-menu__move:disabled {
  opacity: 0.35;
  cursor: default;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--transition-fast);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
