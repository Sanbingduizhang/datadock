<script setup lang="ts">
import AppTooltip from './AppTooltip.vue';

/**
 * 工具栏 / 面板头部的图标按钮。
 * label 同时用于无障碍描述与悬浮提示，避免只靠图标传达含义。
 */
withDefaults(
  defineProps<{
    label: string;
    active?: boolean;
    disabled?: boolean;
    tooltipPlacement?: 'top' | 'bottom';
  }>(),
  {
    active: false,
    disabled: false,
    tooltipPlacement: 'bottom',
  },
);

const emit = defineEmits<{ click: [event: MouseEvent] }>();
</script>

<template>
  <AppTooltip :content="label" :placement="tooltipPlacement">
    <button
      type="button"
      class="icon-btn"
      :class="{ 'is-active': active, 'is-disabled': disabled }"
      :aria-label="label"
      :aria-pressed="active ? 'true' : undefined"
      :disabled="disabled"
      @click="emit('click', $event)"
    >
      <slot />
    </button>
  </AppTooltip>
</template>

<style scoped>
.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  color: var(--text-secondary);
  border-radius: var(--radius-md);
  transition:
    background-color var(--transition-fast),
    color var(--transition-fast);
}

.icon-btn:hover:not(:disabled) {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.icon-btn:active:not(:disabled) {
  background-color: var(--surface-active);
}

.icon-btn.is-active {
  color: var(--primary-color);
  background-color: var(--primary-soft);
}

.icon-btn.is-disabled,
.icon-btn:disabled {
  color: var(--text-tertiary);
  cursor: not-allowed;
  opacity: 0.55;
}
</style>
