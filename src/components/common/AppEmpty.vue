<script setup lang="ts">
import { Inbox } from 'lucide-vue-next';

/** 统一空状态：图标 + 说明 + 可选操作入口 */
withDefaults(
  defineProps<{
    title: string;
    description?: string;
    compact?: boolean;
  }>(),
  {
    description: '',
    compact: false,
  },
);
</script>

<template>
  <div class="empty" :class="{ 'empty--compact': compact }">
    <span class="empty__icon" aria-hidden="true">
      <slot name="icon">
        <Inbox :size="compact ? 20 : 26" />
      </slot>
    </span>
    <p class="empty__title">{{ title }}</p>
    <p v-if="description" class="empty__desc">{{ description }}</p>
    <div v-if="$slots.actions" class="empty__actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 100%;
  min-height: 120px;
  padding: var(--space-6);
  text-align: center;
}

.empty--compact {
  gap: var(--space-1);
  min-height: 88px;
  padding: var(--space-4);
}

.empty__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  opacity: 0.85;
}

.empty__title {
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
}

.empty__desc {
  max-width: 380px;
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  line-height: 1.6;
}

.empty__actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-1);
}
</style>
