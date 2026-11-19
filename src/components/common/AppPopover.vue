<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';

/**
 * 锚定浮层：把内容 Teleport 到 body 并用 fixed 定位。
 *
 * 为什么要 Teleport：顶部菜单栏与工具栏都用了 `backdrop-filter`，
 * 这会创建新的层叠上下文（stacking context），内部元素的 `z-index`
 * 无法超越父级，于是下拉面板会被后面的标签栏/内容区盖住。
 * 移到 body 之后不再受任何祖先的裁剪与层级限制。
 */
const props = withDefaults(
  defineProps<{
    open: boolean;
    /** 触发元素，用于计算位置 */
    anchor?: HTMLElement | null;
    placement?: 'bottom-start' | 'bottom-end';
    minWidth?: number;
    maxHeight?: number;
  }>(),
  {
    anchor: null,
    placement: 'bottom-start',
    minWidth: 180,
    maxHeight: 460,
  },
);

const panelRef = ref<HTMLElement | null>(null);
const top = ref(0);
const left = ref(0);

function updatePosition(): void {
  const anchor = props.anchor;
  if (!anchor) {
    return;
  }

  const rect = anchor.getBoundingClientRect();
  top.value = Math.round(rect.bottom + 4);
  left.value = Math.round(props.placement === 'bottom-end' ? rect.right : rect.left);
}

function reposition(): void {
  if (props.open) {
    updatePosition();
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      updatePosition();
      // 首次渲染后可能因内容高度变化而需要对齐，再算一次
      void nextTick(updatePosition);
    }
  },
);

watch(
  () => props.anchor,
  () => {
    if (props.open) {
      updatePosition();
    }
  },
);

onMounted(() => {
  window.addEventListener('resize', reposition);
  // 捕获阶段监听，祖先容器的滚动（如结果表格）也能跟随
  window.addEventListener('scroll', reposition, true);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', reposition);
  window.removeEventListener('scroll', reposition, true);
});

const panelStyle = computed(() => ({
  top: `${top.value}px`,
  left: `${left.value}px`,
  minWidth: `${props.minWidth}px`,
  maxHeight: `${props.maxHeight}px`,
}));

/** 判断节点是否落在面板内（用于点击外部关闭） */
function contains(target: Node | null): boolean {
  return Boolean(target && panelRef.value?.contains(target));
}

defineExpose({ contains });
</script>

<template>
  <Teleport to="body">
    <Transition name="popover-fade">
      <div
        v-if="open"
        ref="panelRef"
        class="popover"
        :class="`popover--${placement}`"
        :style="panelStyle"
      >
        <slot />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.popover {
  position: fixed;
  z-index: var(--z-popover);
  overflow: auto;
  overscroll-behavior: contain;
}

.popover--bottom-end {
  transform: translateX(-100%);
}

.popover-fade-enter-active,
.popover-fade-leave-active {
  transition: opacity var(--transition-fast);
}

.popover-fade-enter-from,
.popover-fade-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .popover-fade-enter-active,
  .popover-fade-leave-active {
    transition: none;
  }
}
</style>
