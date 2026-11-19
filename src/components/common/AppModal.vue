<script setup lang="ts">
import { X } from 'lucide-vue-next';
import { onBeforeUnmount, ref, watch } from 'vue';

import AppIconButton from './AppIconButton.vue';

/**
 * 通用弹窗：居中显示，支持 Esc 关闭。
 * 默认点击遮罩不关闭，避免误操作丢失输入内容。
 */
const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    title: string;
    width?: number;
    closeOnMask?: boolean;
    hideFooter?: boolean;
  }>(),
  {
    width: 460,
    closeOnMask: false,
    hideFooter: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  close: [];
}>();

const panelRef = ref<HTMLElement | null>(null);

function close(): void {
  emit('update:modelValue', false);
  emit('close');
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.stopPropagation();
    close();
  }
}

function handleMaskClick(): void {
  if (props.closeOnMask) {
    close();
  }
}

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      document.addEventListener('keydown', handleKeydown);
      window.setTimeout(() => panelRef.value?.focus(), 0);
    } else {
      document.removeEventListener('keydown', handleKeydown);
    }
  },
);

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="modelValue" class="modal-mask" @mousedown.self="handleMaskClick">
        <div
          ref="panelRef"
          class="modal"
          role="dialog"
          aria-modal="true"
          :aria-label="title"
          tabindex="-1"
          :style="{ width: `${width}px` }"
        >
          <header class="modal__head">
            <h2 class="modal__title">{{ title }}</h2>
            <AppIconButton label="关闭" @click="close">
              <X :size="16" />
            </AppIconButton>
          </header>

          <div class="modal__body">
            <slot />
          </div>

          <footer v-if="!hideFooter" class="modal__foot">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: var(--z-dialog);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
  background-color: var(--overlay-bg);
  -webkit-backdrop-filter: var(--overlay-blur);
  backdrop-filter: var(--overlay-blur);
}

.modal {
  display: flex;
  flex-direction: column;
  max-height: 80vh;
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-2xl);
  box-shadow: var(--shadow-lg);
  outline: none;
}

.modal__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-4) var(--space-3);
}

.modal__title {
  font-size: var(--font-size-lg);
  font-weight: 600;
}

.modal__body {
  padding: 0 var(--space-4) var(--space-4);
  overflow-y: auto;
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: var(--line-height-normal);
}

.modal__foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--divider-color);
}

.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity var(--transition-normal);
}

.modal-fade-enter-active .modal,
.modal-fade-leave-active .modal {
  transition: transform var(--transition-normal);
}

.modal-fade-enter-from,
.modal-fade-leave-to {
  opacity: 0;
}

.modal-fade-enter-from .modal,
.modal-fade-leave-to .modal {
  transform: translateY(6px) scale(0.99);
}

@media (prefers-reduced-motion: reduce) {
  .modal-fade-enter-active,
  .modal-fade-leave-active,
  .modal-fade-enter-active .modal,
  .modal-fade-leave-active .modal {
    transition: none;
  }
}
</style>
