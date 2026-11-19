<script setup lang="ts">
import { computed } from 'vue';

import { Terminal } from 'lucide-vue-next';

/** 底部状态栏：展示连接状态、当前数据库、标签数量与应用版本 */
const props = withDefaults(
  defineProps<{
    connectionName?: string | null;
    databaseName?: string | null;
    engine?: string;
    tabCount?: number;
    version?: string;
    /** SQL 日志面板是否可见 */
    sqlLogVisible?: boolean;
    /** 本次运行的 SQL 记录数 */
    sqlLogCount?: number;
  }>(),
  {
    connectionName: null,
    databaseName: null,
    engine: 'MySQL',
    tabCount: 0,
    version: '',
    sqlLogVisible: false,
    sqlLogCount: 0,
  },
);

const emit = defineEmits<{ 'toggle-sql-log': [] }>();

const connected = computed(() => Boolean(props.connectionName));
</script>

<template>
  <footer class="statusbar">
    <span class="statusbar__item">
      <span class="statusbar__dot" :class="connected ? 'is-on' : 'is-off'" aria-hidden="true"></span>
      {{ connected ? '已选择连接' : '未连接' }}
    </span>

    <span class="statusbar__divider" aria-hidden="true"></span>
    <span class="statusbar__item">{{ connectionName ?? '—' }}</span>

    <span class="statusbar__divider" aria-hidden="true"></span>
    <span class="statusbar__item">{{ engine }}</span>

    <span class="statusbar__divider" aria-hidden="true"></span>
    <span class="statusbar__item">数据库：{{ databaseName ?? '—' }}</span>

    <span class="statusbar__spacer"></span>

    <button
      type="button"
      class="statusbar__button"
      :class="{ 'is-active': sqlLogVisible }"
      title="查看本次运行执行过的 SQL"
      @click="emit('toggle-sql-log')"
    >
      <Terminal :size="11" />
      SQL 日志{{ sqlLogCount > 0 ? ` ${sqlLogCount}` : '' }}
    </button>
    <span class="statusbar__divider" aria-hidden="true"></span>

    <span class="statusbar__item">标签 {{ tabCount }}</span>
    <span class="statusbar__divider" aria-hidden="true"></span>
    <span class="statusbar__item">v{{ version || '0.0.0' }}</span>
  </footer>
</template>

<style scoped>
.statusbar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: var(--statusbar-height);
  padding: 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  background-color: var(--surface-subtle);
  border-top: 1px solid var(--border-color);
}

.statusbar__item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  white-space: nowrap;
}

.statusbar__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.statusbar__dot.is-on {
  background-color: var(--success-color);
}

.statusbar__dot.is-off {
  background-color: var(--border-strong);
}

.statusbar__button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 18px;
  padding: 0 6px;
  color: var(--text-tertiary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.statusbar__button:hover,
.statusbar__button.is-active {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.statusbar__divider {
  width: 1px;
  height: 10px;
  background-color: var(--border-color);
}

.statusbar__spacer {
  flex: 1;
}
</style>
