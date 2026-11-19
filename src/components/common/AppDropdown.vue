<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';

import AppMenu from './AppMenu.vue';
import AppPopover from './AppPopover.vue';

import type { MenuItem, MenuPlacement } from '@/types/ui';

/**
 * 通用下拉菜单：点击触发器展开，点击外部或按 Esc 关闭。
 *
 * 面板通过 `AppPopover` 挂到 body，避免被工具栏的层叠上下文裁剪/遮挡。
 */
const props = withDefaults(
  defineProps<{
    items: MenuItem[];
    placement?: MenuPlacement;
    disabled?: boolean;
    minWidth?: number;
  }>(),
  {
    placement: 'bottom-start',
    disabled: false,
    minWidth: 180,
  },
);

const emit = defineEmits<{ select: [key: string] }>();

const rootRef = ref<HTMLElement | null>(null);
const triggerRef = ref<HTMLElement | null>(null);
const popoverRef = ref<InstanceType<typeof AppPopover> | null>(null);
const open = ref(false);

function toggle(): void {
  if (props.disabled) {
    return;
  }
  open.value = !open.value;
}

function close(): void {
  open.value = false;
}

function handleSelect(key: string): void {
  emit('select', key);
  close();
}

function handleDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as Node | null;
  if (rootRef.value?.contains(target)) {
    return;
  }
  // 面板已 Teleport 到 body，需要单独判断，否则点击菜单项会被判定为「外部点击」
  if (popoverRef.value?.contains(target)) {
    return;
  }
  close();
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    close();
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
  <div ref="rootRef" class="dropdown">
    <span ref="triggerRef" class="dropdown__trigger">
      <slot :toggle="toggle" :open="open" />
    </span>

    <AppPopover
      ref="popoverRef"
      :open="open"
      :anchor="triggerRef"
      :placement="placement"
      :min-width="minWidth"
    >
      <AppMenu :items="items" :min-width="minWidth" @select="handleSelect" />
    </AppPopover>
  </div>
</template>

<style scoped>
.dropdown {
  position: relative;
  display: inline-flex;
}

.dropdown__trigger {
  display: inline-flex;
}
</style>
