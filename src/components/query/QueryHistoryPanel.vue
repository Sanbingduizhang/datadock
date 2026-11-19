<script setup lang="ts">
import { History, Search, Trash2, X } from 'lucide-vue-next';
import { computed, onBeforeUnmount, ref, watch } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppModal from '@/components/common/AppModal.vue';

import { clearQueryHistory, deleteQueryHistory, listQueryHistory } from '@/api/history';
import { normalizeError } from '@/utils/errors';

import type { QueryHistoryEntry } from '@/types/history';

/**
 * 查询历史下拉面板：按当前连接过滤，点击条目回填 SQL。
 * 历史由后端执行时自动写入，这里只负责展示与维护。
 */
const props = defineProps<{ connectionId?: string }>();

const emit = defineEmits<{ select: [sql: string] }>();

const rootRef = ref<HTMLElement | null>(null);
const open = ref(false);
const loading = ref(false);
const errorMessage = ref('');
const keyword = ref('');
const entries = ref<QueryHistoryEntry[]>([]);
const total = ref(0);

const clearVisible = ref(false);
const clearing = ref(false);

const filtered = computed(() => {
  const keywordValue = keyword.value.trim().toLowerCase();
  if (!keywordValue) {
    return entries.value;
  }
  return entries.value.filter((entry) => entry.sqlText.toLowerCase().includes(keywordValue));
});

function statusLabel(status: string): string {
  switch (status) {
    case 'success':
      return '成功';
    case 'failed':
      return '失败';
    case 'cancelled':
      return '已取消';
    case 'running':
      return '执行中';
    default:
      return status;
  }
}

function formatTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) {
    return '刚刚';
  }
  if (diff < 3_600_000) {
    return `${Math.floor(diff / 60_000)} 分钟前`;
  }
  if (diff < 86_400_000) {
    return `${Math.floor(diff / 3_600_000)} 小时前`;
  }

  const date = new Date(timestamp);
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function previewOf(sql: string): string {
  const compact = sql.split(/\s+/).join(' ').trim();
  return compact.length > 140 ? `${compact.slice(0, 140)}…` : compact;
}

async function load(): Promise<void> {
  loading.value = true;
  errorMessage.value = '';

  try {
    const reply = await listQueryHistory({
      connectionId: props.connectionId,
      limit: 100,
    });
    entries.value = reply.items;
    total.value = reply.total;
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

function toggle(): void {
  open.value = !open.value;
  if (open.value) {
    void load();
  }
}

function close(): void {
  open.value = false;
  keyword.value = '';
}

function handleSelect(entry: QueryHistoryEntry): void {
  emit('select', entry.sqlText);
  close();
}

async function removeEntry(entry: QueryHistoryEntry): Promise<void> {
  try {
    await deleteQueryHistory(entry.id);
    entries.value = entries.value.filter((item) => item.id !== entry.id);
    total.value = Math.max(0, total.value - 1);
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  }
}

async function confirmClear(): Promise<void> {
  clearing.value = true;
  try {
    await clearQueryHistory(props.connectionId);
    entries.value = [];
    total.value = 0;
    clearVisible.value = false;
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  } finally {
    clearing.value = false;
  }
}

function handleDocumentPointerDown(event: PointerEvent): void {
  if (!rootRef.value?.contains(event.target as Node)) {
    close();
  }
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && open.value) {
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

defineExpose({ reload: load });
</script>

<template>
  <div ref="rootRef" class="history">
    <AppButton variant="ghost" size="sm" title="查询历史" @click="toggle">
      <History :size="13" />
      历史
    </AppButton>

    <Transition name="history-fade">
      <div v-if="open" class="history__panel">
        <header class="history__head">
          <label class="history__search">
            <Search :size="12" />
            <input
              v-model="keyword"
              type="text"
              placeholder="搜索 SQL"
              spellcheck="false"
              autocomplete="off"
            />
            <button
              v-if="keyword"
              type="button"
              class="history__search-clear"
              title="清除"
              @click="keyword = ''"
            >
              <X :size="11" />
            </button>
          </label>
        </header>

        <div class="history__body">
          <p v-if="loading" class="history__hint">加载中…</p>
          <p v-else-if="errorMessage" class="history__error">{{ errorMessage }}</p>
          <p v-else-if="filtered.length === 0" class="history__hint">
            暂无查询历史，执行一次 SQL 后会记录在这里
          </p>

          <ul v-else class="history__list">
            <li v-for="entry in filtered" :key="entry.id" class="history__item">
              <button type="button" class="history__entry" @click="handleSelect(entry)">
                <span class="history__sql">{{ previewOf(entry.sqlText) }}</span>
                <span class="history__meta">
                  <span class="history__status" :class="`is-${entry.status}`">
                    {{ statusLabel(entry.status) }}
                  </span>
                  <span>{{ formatTime(entry.startedAt) }}</span>
                  <span>{{ entry.elapsedMs }}ms</span>
                  <span v-if="entry.returnedRows > 0">{{ entry.returnedRows }} 行</span>
                  <span v-else-if="entry.affectedRows">{{ entry.affectedRows }} 行受影响</span>
                </span>
              </button>
              <button
                type="button"
                class="history__remove"
                title="删除这条记录"
                @click.stop="removeEntry(entry)"
              >
                <X :size="11" />
              </button>
            </li>
          </ul>
        </div>

        <footer class="history__foot">
          <span class="history__count">共 {{ total }} 条 · 点击回填</span>
          <span class="history__spacer"></span>
          <button
            type="button"
            class="history__clear"
            :disabled="total === 0"
            @click="clearVisible = true"
          >
            <Trash2 :size="12" />
            清空
          </button>
        </footer>
      </div>
    </Transition>

    <AppModal v-model="clearVisible" title="清空查询历史" :width="400">
      <p class="confirm-text">
        {{ connectionId ? '将清空当前连接的查询历史。' : '将清空全部连接的查询历史。' }}
        此操作不可撤销，已收藏的 SQL 不受影响。
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
.history {
  position: relative;
  display: inline-flex;
}

.history__panel {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: var(--z-dropdown);
  display: flex;
  flex-direction: column;
  width: 380px;
  max-height: 380px;
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
}

.history__head {
  flex: none;
  padding: var(--space-2);
  border-bottom: 1px solid var(--divider-color);
}

.history__search {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: 26px;
  padding: 0 var(--space-2);
  color: var(--text-tertiary);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.history__search input {
  flex: 1;
  min-width: 0;
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  background: none;
  border: none;
  outline: none;
}

.history__search-clear {
  display: inline-flex;
  align-items: center;
  color: var(--text-tertiary);
}

.history__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.history__hint,
.history__error {
  padding: var(--space-3);
  font-size: var(--font-size-xs);
}

.history__hint {
  color: var(--text-tertiary);
}

.history__error {
  color: var(--danger-color);
}

.history__list {
  padding: var(--space-1);
}

.history__item {
  position: relative;
  display: flex;
  align-items: stretch;
}

.history__entry {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  padding: var(--space-2);
  text-align: left;
  border-radius: var(--radius-sm);
}

.history__entry:hover {
  background-color: var(--surface-hover);
}

.history__sql {
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-tertiary);
  font-size: 10px;
}

.history__status {
  color: var(--text-secondary);
}

.history__status.is-success {
  color: var(--success-color);
}

.history__status.is-failed {
  color: var(--danger-color);
}

.history__status.is-cancelled {
  color: var(--warning-color);
}

.history__remove {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  color: transparent;
  border-radius: var(--radius-xs);
}

.history__item:hover .history__remove {
  color: var(--text-tertiary);
}

.history__remove:hover {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}

.history__foot {
  display: flex;
  flex: none;
  align-items: center;
  height: 30px;
  padding: 0 var(--space-2);
  border-top: 1px solid var(--divider-color);
}

.history__count {
  color: var(--text-tertiary);
  font-size: 11px;
}

.history__spacer {
  flex: 1;
}

.history__clear {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  color: var(--text-secondary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.history__clear:hover:not(:disabled) {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}

.history__clear:disabled {
  color: var(--text-tertiary);
  cursor: default;
}

.confirm-text {
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: 1.7;
}

.history-fade-enter-active,
.history-fade-leave-active {
  transition:
    opacity var(--transition-fast),
    transform var(--transition-fast);
}

.history-fade-enter-from,
.history-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
