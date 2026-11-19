<script setup lang="ts">
import { ChevronDown, ChevronRight, CircleAlert, CircleCheck, Trash2, X } from 'lucide-vue-next';
import { computed, ref } from 'vue';

import { useSqlLogStore } from '@/stores/sql-log';
import { copyText } from '@/utils/clipboard';

import type { SqlLogEntry, SqlLogSource } from '@/stores/sql-log';

/**
 * 操作 SQL 日志面板。
 *
 * 展示本次运行期间所有发出的语句（查询、表数据变更、结构变更、Redis 命令），
 * 用于回答「我刚才到底执行了什么」。
 */
const emit = defineEmits<{ close: [] }>();

const logStore = useSqlLogStore();

const filter = ref<'all' | 'failed'>('all');
const expanded = ref<Set<string>>(new Set());

const SOURCE_LABEL: Record<SqlLogSource, string> = {
  query: '查询',
  table: '表数据',
  structure: '结构',
  redis: 'Redis',
  other: '其他',
};

const visibleEntries = computed(() =>
  filter.value === 'failed'
    ? logStore.entries.filter((entry) => entry.status === 'failed')
    : logStore.entries,
);

function toggle(id: string): void {
  const next = new Set(expanded.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  expanded.value = next;
}

function isExpanded(id: string): boolean {
  return expanded.value.has(id);
}

function formatTime(at: number): string {
  const date = new Date(at);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function singleLine(sql: string): string {
  const line = sql.replace(/\s+/g, ' ').trim();
  return line.length > 200 ? `${line.slice(0, 200)}…` : line;
}

async function copy(entry: SqlLogEntry): Promise<void> {
  await copyText(entry.sql);
}

function close(): void {
  emit('close');
}
</script>

<template>
  <section class="sql-log">
    <header class="sql-log__bar">
      <span class="sql-log__title">SQL 日志</span>
      <span class="sql-log__count">{{ visibleEntries.length }} 条</span>
      <span v-if="logStore.failureCount > 0" class="sql-log__failed">
        {{ logStore.failureCount }} 条失败
      </span>

      <div class="sql-log__filter">
        <button
          type="button"
          class="sql-log__filter-item"
          :class="{ 'is-active': filter === 'all' }"
          @click="filter = 'all'"
        >
          全部
        </button>
        <button
          type="button"
          class="sql-log__filter-item"
          :class="{ 'is-active': filter === 'failed' }"
          @click="filter = 'failed'"
        >
          仅失败
        </button>
      </div>

      <span class="sql-log__spacer"></span>

      <button
        type="button"
        class="sql-log__action"
        :disabled="logStore.count === 0"
        title="清空日志"
        @click="logStore.clear()"
      >
        <Trash2 :size="12" />
        清空
      </button>
      <button type="button" class="sql-log__action" title="关闭面板" @click="close">
        <X :size="13" />
      </button>
    </header>

    <div class="sql-log__body">
      <p v-if="visibleEntries.length === 0" class="sql-log__empty">
        还没有执行记录。执行查询、修改表数据或调整表结构后会出现在这里。
      </p>

      <ul v-else class="sql-log__list">
        <li
          v-for="entry in visibleEntries"
          :key="entry.id"
          class="entry"
          :class="{ 'is-failed': entry.status === 'failed' }"
        >
          <button type="button" class="entry__head" @click="toggle(entry.id)">
            <component :is="isExpanded(entry.id) ? ChevronDown : ChevronRight" :size="12" />
            <span class="entry__time">{{ formatTime(entry.at) }}</span>
            <span class="entry__source">{{ SOURCE_LABEL[entry.source] }}</span>
            <CircleCheck v-if="entry.status === 'success'" :size="12" class="entry__ok" />
            <CircleAlert v-else :size="12" class="entry__fail" />
            <span class="entry__sql">{{ singleLine(entry.sql) }}</span>
            <span class="entry__elapsed">{{ entry.elapsedMs }}ms</span>
          </button>

          <div v-if="isExpanded(entry.id)" class="entry__detail">
            <pre class="entry__full">{{ entry.sql }}</pre>
            <p v-if="entry.message" class="entry__message">{{ entry.message }}</p>
            <button type="button" class="entry__copy" @click="copy(entry)">复制语句</button>
          </div>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.sql-log {
  display: flex;
  flex: none;
  flex-direction: column;
  height: 200px;
  background-color: var(--surface-bg);
  border-top: 1px solid var(--border-color);
}

.sql-log__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 30px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.sql-log__title {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  font-weight: 500;
}

.sql-log__count,
.sql-log__failed {
  color: var(--text-tertiary);
  font-size: 11px;
}

.sql-log__failed {
  color: var(--danger-color);
}

.sql-log__filter {
  display: inline-flex;
  gap: 2px;
  padding: 1px;
  background-color: var(--surface-subtle);
  border-radius: var(--radius-sm);
}

.sql-log__filter-item {
  height: 20px;
  padding: 0 7px;
  color: var(--text-tertiary);
  font-size: 10px;
  border-radius: var(--radius-xs);
}

.sql-log__filter-item.is-active {
  color: var(--text-primary);
  background-color: var(--surface-bg);
}

.sql-log__spacer {
  flex: 1;
}

.sql-log__action {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px;
  color: var(--text-tertiary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.sql-log__action:hover:not(:disabled) {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.sql-log__action:disabled {
  opacity: 0.5;
}

.sql-log__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.sql-log__empty {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.sql-log__list {
  display: flex;
  flex-direction: column;
}

.entry {
  border-bottom: 1px solid var(--divider-color);
}

.entry__head {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  height: 26px;
  padding: 0 var(--space-3);
  color: var(--text-secondary);
  font-size: 11px;
  text-align: left;
}

.entry__head:hover {
  background-color: var(--surface-hover);
}

.entry__time {
  flex: none;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.entry__source {
  flex: none;
  padding: 0 5px;
  color: var(--text-tertiary);
  font-size: 10px;
  background-color: var(--surface-subtle);
  border-radius: var(--radius-xs);
}

.entry__ok {
  flex: none;
  color: var(--success-color);
}

.entry__fail {
  flex: none;
  color: var(--danger-color);
}

.entry__sql {
  flex: 1;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.entry.is-failed .entry__sql {
  color: var(--danger-color);
}

.entry__elapsed {
  flex: none;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.entry__detail {
  padding: 0 var(--space-3) var(--space-3);
}

.entry__full {
  padding: var(--space-2);
  overflow: auto;
  max-height: 160px;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  user-select: text;
}

.entry__message {
  margin-top: var(--space-1);
  color: var(--danger-color);
  font-size: 11px;
  line-height: 1.6;
  user-select: text;
}

.entry__copy {
  margin-top: var(--space-1);
  color: var(--primary-color);
  font-size: 11px;
}

.entry__copy:hover {
  text-decoration: underline;
}
</style>
