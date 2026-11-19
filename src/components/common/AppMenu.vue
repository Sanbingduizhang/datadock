<script setup lang="ts">
import { Check } from 'lucide-vue-next';

import type { MenuItem } from '@/types/ui';

/** 下拉菜单面板（纯展示）。定位与开关逻辑由调用方负责。 */
withDefaults(
  defineProps<{
    items: MenuItem[];
    minWidth?: number;
  }>(),
  {
    minWidth: 180,
  },
);

const emit = defineEmits<{ select: [key: string] }>();

function handleClick(item: MenuItem): void {
  if (item.disabled || item.divider) {
    return;
  }
  emit('select', item.key);
}
</script>

<template>
  <div class="menu" role="menu" :style="{ minWidth: `${minWidth}px` }">
    <template v-for="item in items" :key="item.key">
      <div v-if="item.divider" class="menu__divider" role="separator"></div>
      <button
        v-else
        type="button"
        class="menu__item"
        :class="{ 'is-danger': item.danger, 'is-checked': item.checked }"
        role="menuitem"
        :disabled="item.disabled"
        @click="handleClick(item)"
      >
        <span class="menu__check">
          <Check v-if="item.checked" :size="13" />
        </span>
        <span class="menu__label">{{ item.label }}</span>
        <span v-if="item.shortcut" class="menu__shortcut">{{ item.shortcut }}</span>
      </button>
    </template>
  </div>
</template>

<style scoped>
.menu {
  padding: var(--space-1);
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
}

.menu__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: 28px;
  padding: 0 var(--space-2);
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  text-align: left;
  border-radius: var(--radius-sm);
}

.menu__item:hover:not(:disabled) {
  background-color: var(--surface-hover);
}

.menu__item:disabled {
  color: var(--text-tertiary);
  cursor: default;
}

.menu__item.is-danger {
  color: var(--danger-color);
}

.menu__check {
  display: inline-flex;
  width: 14px;
  color: var(--primary-color);
}

.menu__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu__shortcut {
  flex: none;
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.menu__divider {
  height: 1px;
  margin: var(--space-1) var(--space-2);
  background-color: var(--divider-color);
}
</style>
