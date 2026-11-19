<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, TriangleAlert } from 'lucide-vue-next';

import { useToast } from '@/composables/useToast';

import type { ToastType } from '@/composables/useToast';

const { toasts } = useToast();

const ICONS = {
  info: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  error: CircleAlert,
} as const satisfies Record<ToastType, unknown>;
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-live="polite" aria-atomic="false">
      <TransitionGroup name="toast">
        <div v-for="toast in toasts" :key="toast.id" class="toast" :class="`toast--${toast.type}`">
          <component :is="ICONS[toast.type]" :size="14" class="toast__icon" />
          <span class="toast__text">{{ toast.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: var(--space-4);
  bottom: calc(var(--statusbar-height) + var(--space-4));
  z-index: var(--z-toast);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  max-width: 360px;
  padding: 7px var(--space-3);
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  background-color: var(--glass-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
  backdrop-filter: var(--glass-blur);
}

.toast__icon {
  flex: none;
}

.toast--info .toast__icon {
  color: var(--primary-color);
}

.toast--success .toast__icon {
  color: var(--success-color);
}

.toast--warning .toast__icon {
  color: var(--warning-color);
}

.toast--error .toast__icon {
  color: var(--danger-color);
}

.toast__text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.toast-enter-active,
.toast-leave-active {
  transition:
    opacity var(--transition-normal),
    transform var(--transition-normal);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

@media (prefers-reduced-motion: reduce) {
  .toast-enter-active,
  .toast-leave-active {
    transition: none;
  }
}
</style>
