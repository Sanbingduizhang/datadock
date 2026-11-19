<script setup lang="ts">
import { LoaderCircle } from 'lucide-vue-next';

const props = withDefaults(
  defineProps<{
    variant?: 'default' | 'primary' | 'ghost' | 'danger';
    size?: 'sm' | 'md';
    disabled?: boolean;
    loading?: boolean;
    block?: boolean;
    type?: 'button' | 'submit';
  }>(),
  {
    variant: 'default',
    size: 'md',
    disabled: false,
    loading: false,
    block: false,
    type: 'button',
  },
);

const emit = defineEmits<{ click: [event: MouseEvent] }>();

function handleClick(event: MouseEvent): void {
  if (props.disabled || props.loading) {
    event.preventDefault();
    return;
  }
  emit('click', event);
}
</script>

<template>
  <button
    :type="type"
    class="btn"
    :class="[`btn--${variant}`, `btn--${size}`, { 'btn--block': block, 'is-loading': loading }]"
    :disabled="disabled || loading"
    @click="handleClick"
  >
    <LoaderCircle v-if="loading" class="btn__spinner" :size="size === 'sm' ? 13 : 15" />
    <slot />
  </button>
</template>

<style scoped>
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  height: 30px;
  padding: 0 var(--space-3);
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  white-space: nowrap;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast),
    opacity var(--transition-fast);
}

.btn:hover:not(:disabled) {
  background-color: var(--surface-active);
}

.btn:active:not(:disabled) {
  background-color: var(--surface-selected);
}

.btn:disabled {
  opacity: 0.5;
}

.btn--sm {
  height: 26px;
  padding: 0 var(--space-2);
  font-size: var(--font-size-xs);
}

.btn--block {
  width: 100%;
}

.btn--primary {
  color: var(--text-inverse);
  background-color: var(--primary-color);
  border-color: transparent;
}

.btn--primary:hover:not(:disabled) {
  background-color: var(--primary-hover);
}

.btn--primary:active:not(:disabled) {
  background-color: var(--primary-active);
}

.btn--ghost {
  background-color: transparent;
  border-color: transparent;
}

.btn--ghost:hover:not(:disabled) {
  background-color: var(--surface-hover);
}

.btn--danger {
  color: var(--danger-color);
  border-color: var(--danger-color);
  background-color: transparent;
}

.btn--danger:hover:not(:disabled) {
  background-color: var(--danger-soft);
}

.btn__spinner {
  animation: btn-spin 900ms linear infinite;
}

@keyframes btn-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .btn__spinner {
    animation-duration: 2s;
  }
}
</style>
