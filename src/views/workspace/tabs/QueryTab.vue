<script setup lang="ts">
import { CircleAlert, Download, Info, Play, Square, Star, X } from 'lucide-vue-next';
import { computed, defineAsyncComponent, ref } from 'vue';

import { save } from '@tauri-apps/plugin-dialog';

import AppButton from '@/components/common/AppButton.vue';
import AppDropdown from '@/components/common/AppDropdown.vue';
import AppInput from '@/components/common/AppInput.vue';
import AppModal from '@/components/common/AppModal.vue';
import QueryHistoryPanel from '@/components/query/QueryHistoryPanel.vue';
import ResultGrid from '@/components/query/ResultGrid.vue';

import { createSqlFavorite } from '@/api/favorite';
import { exportQueryResult } from '@/api/export';
import { cancelTask, executeSql, redisExecute } from '@/api/query';
import { useToast } from '@/composables/useToast';
import { useAppStore } from '@/stores/app';
import { useConnectionStore } from '@/stores/connection';
import { useTransactionStore } from '@/stores/transaction';
import { useUiStore } from '@/stores/ui';
import { normalizeError } from '@/utils/errors';

import type { ExecuteSqlResponse, StatementResult } from '@/types/query';
import type { MenuItem } from '@/types/ui';
import type { WorkspaceTab } from '@/types/workspace';

// Monaco 体积较大，按需加载：只在打开查询标签时下载
const SqlEditor = defineAsyncComponent(() => import('@/components/query/SqlEditor.vue'));

/**
 * SQL 查询标签：编辑区 + 执行/取消 + 多语句结果集切换 + 查询历史。
 * 编辑区目前为多行文本框（含 Tab 缩进），Monaco 在后续迭代替换。
 */
const props = defineProps<{ tab: WorkspaceTab }>();

const emit = defineEmits<{ 'update-draft': [content: string] }>();

const content = computed(() => props.tab.draft ?? '');
const lineCount = computed(() => (content.value ? content.value.split('\n').length : 0));
const charCount = computed(() => content.value.length);

const executing = ref(false);
const cancelling = ref(false);
const cancelNotice = ref('');
const taskId = ref('');
const response = ref<ExecuteSqlResponse | null>(null);
const errorMessage = ref('');
const activeIndex = ref(0);
/** 最近一次实际执行的 SQL，供「导出结果」重放使用 */
const lastSql = ref('');

const appStore = useAppStore();
const uiStore = useUiStore();
const connectionStore = useConnectionStore();

/** 当前标签是否为 Redis 连接（命令控制台模式） */
const isRedis = computed(
  () =>
    connectionStore.connections.find(
      (item) => item.id === props.tab.context.connectionId,
    )?.sourceType === 'redis',
);
const txStore = useTransactionStore();

const transaction = computed(() => txStore.get(props.tab.id));
const toast = useToast();
const exporting = ref(false);

/** 编辑器设置（未加载完成时使用默认值） */
const editorSettings = computed(
  () =>
    appStore.settings?.editor ?? {
      fontSize: 13,
      tabSize: 2,
      wordWrap: 'off' as const,
    },
);

const favoriteVisible = ref(false);
const favoriteName = ref('');
const savingFavorite = ref(false);

const EXPORT_ITEMS: MenuItem[] = [
  { key: 'csv', label: '导出为 CSV' },
  { key: 'json', label: '导出为 JSON' },
];

const results = computed<StatementResult[]>(() => response.value?.results ?? []);
const activeResult = computed(() => results.value[activeIndex.value] ?? null);
const activeSet = computed(() => activeResult.value?.resultSet ?? null);
const hasOutput = computed(() => response.value !== null || errorMessage.value.length > 0);

/** 语句失败信息（IPC 级错误优先） */
const failureText = computed(() => {
  if (errorMessage.value) {
    return errorMessage.value;
  }
  if (response.value?.error) {
    return `第 ${response.value.error.index} 条语句：${response.value.error.message}`;
  }
  return '';
});

const canExport = computed(
  () =>
    Boolean(response.value && lastSql.value) &&
    !response.value?.error &&
    !response.value?.cancelled &&
    // 多语句脚本无法整体导出（导出按单条只读语句重放）
    results.value.length === 1,
);

const canExecute = computed(
  () =>
    Boolean(props.tab.context.connectionId) &&
    content.value.trim().length > 0 &&
    !executing.value,
);

function resultLabel(item: StatementResult): string {
  if (item.resultSet) {
    return `${item.resultSet.rowCount} 行`;
  }
  if (item.affectedRows !== null) {
    return `影响 ${item.affectedRows} 行`;
  }
  return item.message;
}

function newTaskId(): string {
  const random = Math.random().toString(36).slice(2, 8);
  return `q-${Date.now().toString(36)}-${random}`;
}

async function execute(): Promise<void> {
  if (executing.value) {
    return;
  }

  const connectionId = props.tab.context.connectionId;
  if (!connectionId) {
    errorMessage.value = '当前标签没有关联连接，请从左侧资源树右键新建查询';
    response.value = null;
    return;
  }

  if (content.value.trim().length === 0) {
    errorMessage.value = '请输入要执行的 SQL';
    return;
  }

  const id = newTaskId();
  taskId.value = id;
  lastSql.value = content.value;
  executing.value = true;
  cancelling.value = false;
  cancelNotice.value = '';
  errorMessage.value = '';

  try {
    response.value = isRedis.value
      ? await redisExecute(connectionId, content.value)
      : await executeSql({
          connectionId,
          databaseName: props.tab.context.databaseName,
          sql: content.value,
          taskId: id,
          maxRows: appStore.settings?.query.defaultMaxRows,
          ...(transaction.value ? { transactionId: transaction.value.transactionId } : {}),
        });
    activeIndex.value = 0;
    uiStore.bumpHistory();
  } catch (raw) {
    response.value = null;
    errorMessage.value = normalizeError(raw).message;
  } finally {
    executing.value = false;
    cancelling.value = false;
  }
}

async function handleExport(format: string): Promise<void> {
  const connectionId = props.tab.context.connectionId;
  if (!connectionId || !lastSql.value) {
    return;
  }

  const extension = format === 'json' ? 'json' : 'csv';
  const table = props.tab.context.tableName;
  const base = table ? `${table}-result` : 'query-result';

  let path: string | null = null;
  try {
    path = await save({
      title: '导出查询结果',
      defaultPath: `${base}.${extension}`,
      filters: [{ name: extension.toUpperCase(), extensions: [extension] }],
    });
  } catch (raw) {
    toast.error(normalizeError(raw).message);
    return;
  }

  if (!path) {
    return;
  }

  exporting.value = true;
  try {
    const reply = await exportQueryResult({
      connectionId,
      databaseName: props.tab.context.databaseName,
      sql: lastSql.value,
      format: extension,
      path,
    });

    toast.success(
      reply.truncated
        ? `已导出 ${reply.rowsWritten} 行（达到上限，已截断）`
        : `已导出 ${reply.rowsWritten} 行（导出时会重新执行该查询）`,
    );
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    exporting.value = false;
  }
}

async function beginTransaction(): Promise<void> {
  const connectionId = props.tab.context.connectionId;
  if (!connectionId) {
    toast.warning('请先为该标签关联连接');
    return;
  }

  try {
    await txStore.begin(props.tab.id, connectionId, props.tab.context.databaseName);
    toast.success('事务已开启，后续语句将在该事务中执行');
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

async function finishTransaction(action: 'commit' | 'rollback'): Promise<void> {
  try {
    const message =
      action === 'commit' ? await txStore.commit(props.tab.id) : await txStore.rollback(props.tab.id);
    toast.success(message);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

function openFavoriteDialog(): void {
  if (content.value.trim().length === 0) {
    toast.warning('当前没有可收藏的 SQL');
    return;
  }
  favoriteName.value = props.tab.title || '未命名查询';
  favoriteVisible.value = true;
}

async function saveFavorite(): Promise<void> {
  if (savingFavorite.value) {
    return;
  }

  savingFavorite.value = true;
  try {
    await createSqlFavorite({
      name: favoriteName.value.trim(),
      sqlText: content.value,
      connectionId: props.tab.context.connectionId ?? null,
      databaseName: props.tab.context.databaseName ?? null,
    });
    favoriteVisible.value = false;
    uiStore.bumpFavorites();
    toast.success('已加入收藏');
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    savingFavorite.value = false;
  }
}

async function cancel(): Promise<void> {
  if (!executing.value || cancelling.value) {
    return;
  }

  cancelling.value = true;
  cancelNotice.value = '正在请求服务端中断…';

  try {
    const reply = await cancelTask(taskId.value);
    cancelNotice.value = reply.cancelled
      ? '已发送取消指令，等待服务端中断…'
      : '查询可能已经结束，未能取消';
  } catch (raw) {
    cancelNotice.value = normalizeError(raw).message;
  } finally {
    cancelling.value = false;
  }
}

function applyHistory(sql: string): void {
  emit('update-draft', sql);
  clearOutput();
}

function clearOutput(): void {
  response.value = null;
  errorMessage.value = '';
  cancelNotice.value = '';
  activeIndex.value = 0;
}

defineExpose({ execute });
</script>

<template>
  <section class="query-tab">
    <div class="query-tab__context">
      <span class="context-item">
        连接：<strong>{{ tab.context.connectionName ?? '未选择' }}</strong>
      </span>
      <span class="context-divider" aria-hidden="true"></span>
      <span class="context-item">
        数据库：<strong>{{ tab.context.databaseName ?? '默认' }}</strong>
      </span>

      <span class="query-tab__spacer"></span>

      <span v-if="!tab.context.connectionId" class="query-tab__note">
        <Info :size="12" />
        请从左侧连接右键新建查询
      </span>

      <template v-if="tab.context.connectionId && !isRedis">
        <template v-if="transaction">
          <span class="tx-badge" title="当前标签的事务尚未提交">事务中</span>
          <AppButton variant="ghost" size="sm" title="提交事务" @click="finishTransaction('commit')">
            提交
          </AppButton>
          <AppButton variant="ghost" size="sm" title="回滚事务" @click="finishTransaction('rollback')">
            回滚
          </AppButton>
        </template>
        <AppButton v-else variant="ghost" size="sm" title="开启手动事务" @click="beginTransaction">
          开始事务
        </AppButton>
      </template>

      <AppButton variant="ghost" size="sm" title="收藏当前 SQL" @click="openFavoriteDialog">
        <Star :size="13" />
        收藏
      </AppButton>

      <QueryHistoryPanel :connection-id="tab.context.connectionId" @select="applyHistory" />

      <AppButton
        v-if="executing"
        variant="danger"
        size="sm"
        :loading="cancelling"
        title="取消执行（Esc）"
        @click="cancel"
      >
        <Square v-if="!cancelling" :size="11" />
        停止
      </AppButton>
      <AppButton v-else variant="primary" size="sm" :disabled="!canExecute" @click="execute">
        <Play :size="13" />
        执行 (⌘↵)
      </AppButton>
    </div>

    <div class="query-tab__editor" :class="{ 'is-compact': hasOutput }">
      <SqlEditor
        :model-value="content"
        :theme="appStore.monacoTheme"
        :font-size="editorSettings.fontSize"
        :tab-size="editorSettings.tabSize"
        :word-wrap="editorSettings.wordWrap === 'on'"
        @update:model-value="emit('update-draft', $event)"
        @execute="execute"
        @cancel="cancel"
      />
    </div>

    <footer class="query-tab__foot">
      <span>{{ lineCount }} 行</span>
      <span class="foot-divider" aria-hidden="true"></span>
      <span>{{ charCount }} 字符</span>
      <span class="query-tab__foot-spacer"></span>
      <span v-if="executing && cancelNotice" class="foot-note">{{ cancelNotice }}</span>
      <span v-else-if="executing" class="foot-note">正在执行…</span>
      <span v-else-if="response">{{ response.message }} · {{ response.elapsedMs }}ms</span>
      <span v-else>⌘/Ctrl + Enter 执行，Esc 取消</span>
    </footer>

    <AppModal v-model="favoriteVisible" title="收藏 SQL" :width="420">
      <label class="favorite-field">
        <span class="favorite-field__label">名称</span>
        <AppInput v-model="favoriteName" placeholder="例如：每日活跃用户" size="sm" />
      </label>
      <p class="favorite-hint">收藏的是当前编辑区内容，可在左侧「收藏」面板中随时取用。</p>
      <template #footer>
        <AppButton size="sm" @click="favoriteVisible = false">取消</AppButton>
        <AppButton
          variant="primary"
          size="sm"
          :loading="savingFavorite"
          :disabled="favoriteName.trim().length === 0"
          @click="saveFavorite"
        >
          收藏
        </AppButton>
      </template>
    </AppModal>

    <div v-if="hasOutput" class="output">
      <header class="output__head">
        <span class="output__title">结果</span>
        <span v-if="response?.cancelled" class="output__warning">已取消</span>
        <span v-else-if="response?.truncated" class="output__warning">
          结果已截断（达到行数上限）
        </span>
        <span class="output__spacer"></span>
        <AppDropdown
          v-if="canExport"
          :items="EXPORT_ITEMS"
          placement="bottom-end"
          @select="handleExport"
        >
          <template #default="{ toggle }">
            <button
              type="button"
              class="output__action"
              :disabled="exporting"
              :title="exporting ? '导出中…' : '导出本次结果'"
              @click="toggle"
            >
              <Download :size="12" />
              {{ exporting ? '导出中…' : '导出' }}
            </button>
          </template>
        </AppDropdown>
        <button type="button" class="output__close" title="清空结果" @click="clearOutput">
          <X :size="13" />
        </button>
      </header>

      <div v-if="results.length > 1" class="output__tabs" role="tablist">
        <button
          v-for="(item, index) in results"
          :key="item.index"
          type="button"
          class="output__tab"
          :class="{ 'is-active': index === activeIndex }"
          role="tab"
          :aria-selected="index === activeIndex"
          :title="item.sqlPreview"
          @click="activeIndex = index"
        >
          结果 {{ item.index }}
          <span class="output__tab-meta">{{ resultLabel(item) }}</span>
        </button>
      </div>

      <div v-if="failureText" class="output__error">
        <CircleAlert :size="14" />
        <span>{{ failureText }}</span>
      </div>

      <ResultGrid v-if="activeSet" class="output__grid" :set="activeSet" />

      <div v-else-if="!failureText" class="output__empty">
        {{ activeResult?.message ?? response?.message }}
        <span v-if="activeResult?.sqlPreview" class="output__sql">{{ activeResult.sqlPreview }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.query-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--surface-bg);
}

.query-tab__context {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  border-bottom: 1px solid var(--divider-color);
}

.tx-badge {
  flex: none;
  padding: 1px 6px;
  color: var(--warning-color);
  font-size: 10px;
  background-color: var(--warning-soft);
  border-radius: var(--radius-xs);
}

.context-item strong {
  color: var(--text-secondary);
  font-weight: 500;
}

.context-divider {
  width: 1px;
  height: 10px;
  background-color: var(--border-color);
}

.query-tab__spacer {
  flex: 1;
}

.query-tab__note {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.query-tab__editor {
  flex: 1;
  min-height: 80px;
  overflow: hidden;
}

.query-tab__editor.is-compact {
  flex: 1 1 40%;
}

.query-tab__foot {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 24px;
  padding: 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  border-top: 1px solid var(--divider-color);
}

.foot-divider {
  width: 1px;
  height: 9px;
  background-color: var(--border-color);
}

.query-tab__foot-spacer {
  flex: 1;
}

.foot-note {
  color: var(--warning-color);
}

.favorite-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.favorite-field__label {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.favorite-hint {
  margin-top: var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  line-height: 1.6;
}

.output {
  display: flex;
  flex: 0 0 45%;
  flex-direction: column;
  min-height: 0;
  border-top: 1px solid var(--border-color);
}

.output__head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 28px;
  padding: 0 var(--space-2) 0 var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  border-bottom: 1px solid var(--divider-color);
}

.output__title {
  color: var(--text-primary);
  font-weight: 500;
}

.output__warning {
  color: var(--warning-color);
  font-size: 11px;
}

.output__spacer {
  flex: 1;
}

.output__action {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px;
  color: var(--text-tertiary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.output__action:hover:not(:disabled) {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.output__action:disabled {
  cursor: progress;
}

.output__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
}

.output__close:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.output__tabs {
  display: flex;
  flex: none;
  gap: 2px;
  padding: 0 var(--space-2);
  overflow-x: auto;
  border-bottom: 1px solid var(--divider-color);
}

.output__tabs::-webkit-scrollbar {
  height: 0;
}

.output__tab {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  height: 26px;
  padding: 0 var(--space-2);
  color: var(--text-tertiary);
  font-size: 11px;
  border-bottom: 2px solid transparent;
}

.output__tab:hover {
  color: var(--text-secondary);
}

.output__tab.is-active {
  color: var(--text-primary);
  border-bottom-color: var(--primary-color);
}

.output__tab-meta {
  color: var(--text-tertiary);
  font-size: 10px;
}

.output__grid {
  flex: 1;
  min-height: 0;
}

.output__error {
  display: flex;
  flex: none;
  align-items: flex-start;
  gap: 6px;
  padding: var(--space-3);
  color: var(--danger-color);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  user-select: text;
}

.output__empty {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.output__sql {
  display: block;
  margin-top: 4px;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
  opacity: 0.7;
}
</style>
