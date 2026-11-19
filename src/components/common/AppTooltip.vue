<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';

/**
 * 轻量提示气泡：通过 Teleport 挂载到 body，避免被滚动容器裁剪。
 * 仅用于补充说明，不作为唯一的信息来源。
 */
const props = withDefaults(
  defineProps<{
    content: string;
    placement?: 'top' | 'bottom';
    delay?: number;
    disabled?: boolean;
  }>(),
  {
    placement: 'bottom',
    delay: 260,
    disabled: false,
  },
);

const triggerRef = ref<HTMLElement | null>(null);
const visible = ref(false);
const position = ref({ x: 0, y: 0 });

let timer: number | undefined;

const tooltipStyle = computed(() => ({
  left: `${position.value.x}px`,
  top: `${position.value.y}px`,
}));

function updatePosition(): void {
  const element = triggerRef.value;
  if (!element) {
    return;
  }
  const rect = element.getBoundingClientRect();
  position.value = {
    x: rect.left + rect.width / 2,
    y: props.placement === 'top' ? rect.top - 6 : rect.bottom + 6,
  };
}

function show(): void {
  if (props.disabled || !props.content) {
    return;
  }
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    updatePosition();
    visible.value = true;
  }, props.delay);
}

function hide(): void {
  window.clearTimeout(timer);
  visible.value = false;
}

onBeforeUnmount(() => {
  window.clearTimeout(timer);
});
</script>

<template>
  <span
    ref="triggerRef"
    class="tooltip-trigger"
    @mouseenter="show"
    @mouseleave="hide"
    @focusin="show"
    @focusout="hide"
  >
    <slot />
  </span>

  <Teleport to="body">
    <div
      v-if="visible"
      class="tooltip"
      :class="`tooltip--${placement}`"
      :style="tooltipStyle"
      role="tooltip"
    >
      {{ content }}
    </div>
  </Teleport>
</template>

<style scoped>
.tooltip-trigger {
  display: inline-flex;
  min-width: 0;
}

.tooltip {
  position: fixed;
  z-index: var(--z-popover);
  max-width: 260px;
  padding: 5px 8px;
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  line-height: 1.4;
  white-space: nowrap;
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  pointer-events: none;
}

.tooltip--bottom {
  transform: translate(-50%, 0);
}

.tooltip--top {
  transform: translate(-50%, -100%);
}
</style>
