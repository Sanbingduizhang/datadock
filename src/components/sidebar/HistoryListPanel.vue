<script setup lang="ts">
import { Trash2 } from 'lucide-vue-next';
import { computed, onMounted, ref, watch } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppEmpty from '@/components/common/AppEmpty.vue';
import AppModal from '@/components/common/AppModal.vue';

import { clearQueryHistory, listQueryHistory } from '@/api/history';
import { useToast } from '@/composables/useToast';
import { useUiStore } from '@/stores/ui';
import { normalizeError } from '@/utils/errors';
import { compactSql, formatRelativeTime } from '@/utils/format';

import type { QueryHistoryEntry } from '@/types/history';

/** 侧栏「历史」面板：最近 200 条执行记录，点击回填到新的查询标签。 */
const props = defineProps<{ keyword: string; active: boolean }>();

const emit = defineEmits<{
  'open-sql': [payload: { sql: string; connectionId?: string; databaseName?: string }];
}>();

const uiStore = useUiStore();
const toast = useToast();
const loading = ref(false);
const errorMessage = ref('');
const entries = ref<QueryHistoryEntry[]>([]);
const clearVisible = ref(false);
const clearing = ref(false);

const filtered = computed(() => {
  const keyword = props.keyword.trim().toLowerCase();
  if (!keyword) {
    return entries.value;
  }
  return entries.value.filter((entry) => entry.sqlText.toLowerCase().includes(keyword));
});

function statusClass(status: string): string {
  return `status-dot is-${status}`;
}

async function load(): Promise<void> {
  loading.value = true;
  errorMessage.value = '';
  try {
    const reply = await listQueryHistory({ limit: 200 });
    entries.value = reply.items;
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

function open(entry: QueryHistoryEntry): void {
  emit('open-sql', {
    sql: entry.sqlText,
    ...(entry.connectionId ? { connectionId: entry.connectionId } : {}),
    ...(entry.databaseName ? { databaseName: entry.databaseName } : {}),
  });
}

async function confirmClear(): Promise<void> {
  clearing.value = true;
  try {
    const removed = await clearQueryHistory();
    entries.value = [];
    clearVisible.value = false;
    toast.success(`已清空 ${removed} 条历史记录`);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    clearing.value = false;
  }
}

watch(
  () => props.active,
  (active) => {
    if (active) {
      void load();
    }
  },
);

watch(
  () => uiStore.historyVersion,
  () => {
    if (props.active) {
      void load();
    }
  },
);

onMounted(() => {
  if (props.active) {
    void load();
  }
});
</script>

<template>
  <div class="panel">
    <div class="panel__head">
      <span class="panel__count">{{ entries.length }} 条记录</span>
      <span class="panel__spacer"></span>
      <button
        type="button"
        class="panel__action"
        :disabled="entries.length === 0"
        title="清空全部历史"
        @click="clearVisible = true"
      >
        <Trash2 :size="12" />
        清空
      </button>
    </div>

    <p v-if="loading" class="panel__hint">加载中…</p>
    <p v-else-if="errorMessage" class="panel__error">{{ errorMessage }}</p>

    <AppEmpty
      v-else-if="filtered.length === 0"
      title="暂无查询历史"
      description="执行一次 SQL 后会记录在这里，点击记录可回填到新的查询标签。"
      compact
    />

    <ul v-else class="list">
      <li v-for="entry in filtered" :key="entry.id" class="item">
        <button type="button" class="item__main" :title="entry.sqlText" @click="open(entry)">
          <span class="item__sql">{{ compactSql(entry.sqlText) }}</span>
          <span class="item__meta">
            <span :class="statusClass(entry.status)"></span>
            <span>{{ formatRelativeTime(entry.startedAt) }}</span>
            <span>{{ entry.elapsedMs }}ms</span>
            <span v-if="entry.returnedRows > 0">{{ entry.returnedRows }} 行</span>
            <span v-else-if="entry.affectedRows">{{ entry.affectedRows }} 行受影响</span>
            <span v-if="entry.databaseName" class="item__db">{{ entry.databaseName }}</span>
          </span>
        </button>
      </li>
    </ul>

    <AppModal v-model="clearVisible" title="清空查询历史" :width="400">
      <p class="confirm-text">
        将删除全部 {{ entries.length }} 条查询历史，此操作不可撤销；SQL 收藏不受影响。
      </p>
      <template #footer>
        <AppButton size="sm" @click="clearVisible = false">取消</AppButton>
        <AppButton variant="danger" size="sm" :loading="clearing" @click="confirmClear">
          清空
        </AppButton>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.panel__head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 26px;
  padding: 0 var(--space-2) 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  border-bottom: 1px solid var(--divider-color);
}

.panel__spacer {
  flex: 1;
}

.panel__action {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 5px;
  color: var(--text-tertiary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.panel__action:hover:not(:disabled) {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}

.panel__action:disabled {
  color: var(--text-tertiary);
  opacity: 0.5;
  cursor: default;
}

.panel__hint,
.panel__error {
  padding: var(--space-3);
  font-size: var(--font-size-xs);
}

.panel__hint {
  color: var(--text-tertiary);
}

.panel__error {
  color: var(--danger-color);
}

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.item + .item {
  border-top: 1px solid var(--divider-color);
}

.item__main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  width: 100%;
  padding: 6px var(--space-3);
  text-align: left;
}

.item__main:hover {
  background-color: var(--surface-hover);
}

.item__sql {
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-tertiary);
  font-size: 10px;
}

.item__db {
  overflow: hidden;
  max-width: 90px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: var(--text-tertiary);
}

.status-dot.is-success {
  background-color: var(--success-color);
}

.status-dot.is-failed {
  background-color: var(--danger-color);
}

.status-dot.is-cancelled {
  background-color: var(--warning-color);
}

.confirm-text {
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: 1.7;
}
</style>
