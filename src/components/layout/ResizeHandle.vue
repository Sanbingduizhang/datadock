<script setup lang="ts">
import { ref } from 'vue';

/**
 * 面板拖拽分隔条。
 * axis='x' 表示拖动改变宽度（竖直分隔条），axis='y' 表示拖动改变高度。
 */
const props = withDefaults(
  defineProps<{
    axis: 'x' | 'y';
    /** 无障碍描述 */
    label: string;
  }>(),
  { axis: 'x' },
);

const emit = defineEmits<{
  resize: [delta: number];
  resizeend: [];
}>();

const dragging = ref(false);
let lastPosition = 0;

function handlePointerDown(event: PointerEvent): void {
  event.preventDefault();
  dragging.value = true;
  lastPosition = props.axis === 'x' ? event.clientX : event.clientY;

  const target = event.currentTarget as HTMLElement;
  target.setPointerCapture(event.pointerId);
  document.body.style.cursor = props.axis === 'x' ? 'col-resize' : 'row-resize';
}

function handlePointerMove(event: PointerEvent): void {
  if (!dragging.value) {
    return;
  }

  const current = props.axis === 'x' ? event.clientX : event.clientY;
  const delta = current - lastPosition;
  if (delta === 0) {
    return;
  }

  lastPosition = current;
  emit('resize', delta);
}

function handlePointerUp(event: PointerEvent): void {
  if (!dragging.value) {
    return;
  }

  dragging.value = false;
  document.body.style.cursor = '';

  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) {
    target.releasePointerCapture(event.pointerId);
  }
  emit('resizeend');
}
</script>

<template>
  <div
    class="resize-handle"
    :class="[`resize-handle--${axis}`, { 'is-dragging': dragging }]"
    role="separator"
    :aria-label="label"
    :aria-orientation="axis === 'x' ? 'vertical' : 'horizontal'"
    @pointerdown="handlePointerDown"
    @pointermove="handlePointerMove"
    @pointerup="handlePointerUp"
    @pointercancel="handlePointerUp"
  ></div>
</template>

<style scoped>
.resize-handle {
  position: relative;
  z-index: 1;
  flex: none;
  background-color: transparent;
}

.resize-handle--x {
  width: 5px;
  cursor: col-resize;
}

.resize-handle--y {
  height: 5px;
  cursor: row-resize;
}

.resize-handle::after {
  position: absolute;
  content: '';
  background-color: transparent;
  transition: background-color var(--transition-fast);
}

.resize-handle--x::after {
  top: 0;
  bottom: 0;
  left: 2px;
  width: 1px;
}

.resize-handle--y::after {
  top: 2px;
  right: 0;
  left: 0;
  height: 1px;
}

.resize-handle:hover::after,
.resize-handle.is-dragging::after {
  background-color: var(--primary-color);
}
</style>
