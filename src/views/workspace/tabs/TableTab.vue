<script setup lang="ts">
import {
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Download,
  Eye,
  Filter,
  Pencil,
  Plus,
  Save,
  RefreshCw,
  Trash2,
} from 'lucide-vue-next';
import { computed, onMounted, ref, watch } from 'vue';
import { save } from '@tauri-apps/plugin-dialog';

import AppButton from '@/components/common/AppButton.vue';
import AppDropdown from '@/components/common/AppDropdown.vue';
import AppModal from '@/components/common/AppModal.vue';
import ResultGrid from '@/components/query/ResultGrid.vue';
import ColumnMenu from '@/components/table/ColumnMenu.vue';
import FilterPanel from '@/components/table/FilterPanel.vue';

import { exportTableData } from '@/api/export';
import { getTableSchema } from '@/api/schema';
import { executeBatchSql, executeSql } from '@/api/query';
import { getTableData } from '@/api/table';
import { useToast } from '@/composables/useToast';
import { useAppStore } from '@/stores/app';
import { normalizeError } from '@/utils/errors';
import { buildDeleteSql, buildInsertSql, buildUpdateSql, canEditRow } from '@/utils/table-edit';
import { validateCellValue } from '@/utils/table-validate';
import { copyText } from '@/utils/clipboard';
import { loadTableViewPrefs, saveTableViewPrefs, tableViewScope } from '@/utils/table-view-prefs';

import type { DatabaseValue, QueryColumn } from '@/types/query';
import type { MenuItem } from '@/types/ui';
import type { InsertField, RowEditContext } from '@/utils/table-edit';
import type { TableDataResponse } from '@/types/table';
import type { WorkspaceTab } from '@/types/workspace';

/**
 * 表数据标签：分页读取 + 列头排序 + 条件筛选 + 行详情 + 导出。
 * 行编辑通过「主键定位 + LIMIT 1」生成单行 UPDATE/DELETE，执行前展示 SQL 供确认。
 */
const props = defineProps<{ tab: WorkspaceTab }>();

const PAGE_SIZES = [50, 100, 200, 500, 1000];

const loading = ref(false);
const errorMessage = ref('');
const data = ref<TableDataResponse | null>(null);
const appStore = useAppStore();
const page = ref(1);
const pageSize = ref(appStore.settings?.data.defaultPageSize ?? 100);

/** 防止并发请求堆叠：首次打开时挂载、分页、排序、筛选可能同时触发 load */
let loadSeq = 0;

/** 设置里可能填了非预设值，补进下拉避免显示为空 */
const pageSizeOptions = computed(() =>
  Array.from(new Set([...PAGE_SIZES, pageSize.value])).sort((a, b) => a - b),
);

const filterOpen = ref(false);
const appliedFilter = ref('');

/** 「查看 SQL」弹窗 */
const showSqlVisible = ref(false);

async function copyCurrentSql(): Promise<void> {
  const ok = await copyText(currentSelectSql.value);
  if (ok) {
    toast.success('已复制当前查询语句');
  } else {
    toast.error('复制失败，请手动选择文本复制');
  }
}

/** 当前视图对应的 SELECT（含分页与排序、筛选），用于学习与分享 */
const currentSelectSql = computed<string>(() => {
  const { databaseName, tableName } = props.tab.context;
  if (!databaseName || !tableName) {
    return '';
  }
  const q = (name: string) => `\`${name.replace(/`/g, '``')}\``;
  const where = appliedFilter.value ? ` WHERE ${appliedFilter.value}` : '';
  const orderColumn = sortColumn.value ?? '';
  const order = orderColumn ? ` ORDER BY ${q(orderColumn)}${sortDesc.value ? ' DESC' : ' ASC'}` : '';
  const limit = Math.max(pageSize.value, 1);
  return `SELECT * FROM ${q(databaseName)}.${q(tableName)}${where}${order} LIMIT ${limit} OFFSET ${(page.value - 1) * limit};`;
});

/** 列元数据（可空/完整类型），用于单元格类型校验 */
const schemaColumns = ref<Record<string, { columnType: string; nullable: boolean }>>({});

async function ensureSchema(): Promise<void> {
  const { connectionId, databaseName, tableName } = props.tab.context;
  if (!connectionId || !databaseName || !tableName) {
    return;
  }

  try {
    const schema = await getTableSchema(connectionId, databaseName, tableName);
    schemaColumns.value = Object.fromEntries(
      schema.columns.map((column) => [
        column.name,
        { columnType: column.columnType, nullable: column.nullable },
      ]),
    );
  } catch {
    // 读取失败时降级为不校验，由数据库报错兜底
    schemaColumns.value = {};
  }
}

/* ---------- 列显示与顺序 ---------- */

const columnOrder = ref<string[]>([]);
const hiddenColumns = ref<string[]>([]);
const columnWidths = ref<Record<string, number>>({});

function prefsScope(): string | null {
  const { connectionId, databaseName, tableName } = props.tab.context;
  if (!connectionId || !databaseName || !tableName) {
    return null;
  }
  return tableViewScope(connectionId, databaseName, tableName);
}

function loadPrefs(): void {
  const scope = prefsScope();
  if (!scope) {
    return;
  }

  const prefs = loadTableViewPrefs(scope);
  if (prefs) {
    columnOrder.value = prefs.order;
    hiddenColumns.value = prefs.hidden;
    columnWidths.value = prefs.widths;
  }
}

function persistPrefs(): void {
  const scope = prefsScope();
  if (!scope) {
    return;
  }

  saveTableViewPrefs(scope, {
    order: columnOrder.value,
    hidden: hiddenColumns.value,
    widths: columnWidths.value,
  });
}

/** 保留已有顺序，把新列追加到末尾 */
function syncColumns(columns: QueryColumn[]): void {
  const names = columns.map((column) => column.name);
  const kept = columnOrder.value.filter((name) => names.includes(name));
  const added = names.filter((name) => !kept.includes(name));
  const next = [...kept, ...added];

  if (next.join('|') !== columnOrder.value.join('|')) {
    columnOrder.value = next;
  }

  const visibleHidden = hiddenColumns.value.filter((name) => names.includes(name));
  if (visibleHidden.length !== hiddenColumns.value.length) {
    hiddenColumns.value = visibleHidden;
  }
}

const orderedColumns = computed<QueryColumn[]>(() => {
  const all = data.value?.columns ?? [];
  if (columnOrder.value.length === 0) {
    return all;
  }

  const byName = new Map(all.map((column) => [column.name, column]));
  return columnOrder.value
    .map((name) => byName.get(name))
    .filter((column): column is QueryColumn => Boolean(column));
});

const viewColumns = computed(() =>
  orderedColumns.value.filter((column) => !hiddenColumns.value.includes(column.name)),
);

const columnIndexes = computed(() =>
  viewColumns.value.map((column) =>
    (data.value?.columns ?? []).findIndex((item) => item.name === column.name),
  ),
);

const viewRows = computed<DatabaseValue[][]>(() => {
  const rows = data.value?.rows ?? [];
  return rows.map((row) =>
    columnIndexes.value.map((index) => row[index] ?? { type: 'null' as const }),
  );
});

function toggleColumn(name: string): void {
  hiddenColumns.value = hiddenColumns.value.includes(name)
    ? hiddenColumns.value.filter((item) => item !== name)
    : [...hiddenColumns.value, name];
  persistPrefs();
}

function moveColumn(name: string, direction: -1 | 1): void {
  const order = [...columnOrder.value];
  const index = order.indexOf(name);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= order.length) {
    return;
  }
  [order[index], order[target]] = [order[target]!, order[index]!];
  columnOrder.value = order;
  persistPrefs();
}

function resetColumns(): void {
  columnOrder.value = (data.value?.columns ?? []).map((column) => column.name);
  hiddenColumns.value = [];
  columnWidths.value = {};
  persistPrefs();
}

function handleColumnResize(name: string, width: number): void {
  columnWidths.value = { ...columnWidths.value, [name]: Math.round(width) };
  persistPrefs();
}

/* ---------- 待保存变更 ---------- */

interface PendingChange {
  kind: 'update' | 'delete';
  original: DatabaseValue[];
  next: (string | null)[];
}

const pending = ref<Record<string, PendingChange>>({});
const pendingCount = computed(() => Object.keys(pending.value).length);

const batchVisible = ref(false);
const batchExecuting = ref(false);
const batchPendingSql = ref('');

function rowKeyOf(row: DatabaseValue[]): string | null {
  const current = data.value;
  if (!current || current.primaryKey.length === 0) {
    return null;
  }

  const parts: string[] = [];
  for (const key of current.primaryKey) {
    const index = current.columns.findIndex((column) => column.name === key);
    const cell = row[index];
    if (!cell || cell.type === 'null') {
      return null;
    }
    parts.push(cell.value ?? '');
  }

  return parts.join('\u0001');
}

const highlightRows = computed<number[]>(() => {
  const rows = data.value?.rows ?? [];
  const result: number[] = [];
  rows.forEach((row, index) => {
    const key = rowKeyOf(row);
    if (key && pending.value[key]) {
      result.push(index);
    }
  });
  return result;
});

const editColumns = computed(() =>
  (data.value?.columns ?? []).map((column) => ({
    name: column.name,
    columnType: column.typeName,
  })),
);

const cellOverrides = computed<Record<string, string | null>>(() => {
  const current = data.value;
  if (!current) {
    return {};
  }

  const overrides: Record<string, string | null> = {};
  current.rows.forEach((row, rowIndex) => {
    const key = rowKeyOf(row);
    const change = key ? pending.value[key] : null;
    if (!change || change.kind !== 'update') {
      return;
    }

    viewColumns.value.forEach((column, colIndex) => {
      const originalIndex = current.columns.findIndex((item) => item.name === column.name);
      if (originalIndex < 0) {
        return;
      }
      overrides[`${rowIndex}:${colIndex}`] = change.next[originalIndex] ?? null;
    });
  });

  return overrides;
});

/** 双击单元格提交：校验后进入待保存队列 */
function handleCellCommit(rowIndex: number, columnIndex: number, value: string): void {
  const current = data.value;
  if (!current) {
    return;
  }

  const column = viewColumns.value[columnIndex];
  const row = current.rows[rowIndex];
  if (!column || !row) {
    return;
  }

  const meta = schemaColumns.value[column.name] ?? {
    columnType: column.typeName,
    nullable: true,
  };
  const error = validateCellValue({
    columnType: meta.columnType,
    nullable: meta.nullable,
    value,
  });
  if (error) {
    toast.warning(`${column.name}：${error}`);
    return;
  }

  const key = rowKeyOf(row);
  if (!key) {
    toast.warning('该表没有可用主键，无法直接编辑单元格');
    return;
  }

  const existing = pending.value[key];
  const base = existing
    ? [...existing.next]
    : row.map((cell) => (cell.type === 'null' ? null : (cell.value ?? '')));

  const originalIndex = current.columns.findIndex((item) => item.name === column.name);
  if (originalIndex < 0) {
    return;
  }
  base[originalIndex] = value;

  const unchanged = base.every((next, index) => {
    const original = row[index];
    if (!original || original.type === 'null') {
      return next === null;
    }
    return next !== null && next === original.value;
  });

  if (unchanged) {
    const next = { ...pending.value };
    delete next[key];
    pending.value = next;
    return;
  }

  pending.value = {
    ...pending.value,
    [key]: { kind: 'update', original: row, next: base },
  };
}

const batchStatements = computed<string[]>(() => {
  const current = data.value;
  if (!current) {
    return [];
  }

  const statements: string[] = [];
  for (const change of Object.values(pending.value)) {
    const context: RowEditContext = {
      databaseName: props.tab.context.databaseName ?? '',
      tableName: props.tab.context.tableName ?? '',
      columns: editColumns.value,
      primaryKey: current.primaryKey,
      original: change.original,
      next: change.next,
    };

    const sql = change.kind === 'delete' ? buildDeleteSql(context) : buildUpdateSql(context);
    if (sql) {
      statements.push(sql);
    }
  }

  return statements;
});
const insertVisible = ref(false);
const insertFields = ref<InsertField[]>([]);
const inserting = ref(false);
const sortColumn = ref<string | null>(null);
const sortDesc = ref(false);

const detailVisible = ref(false);
const detailIndex = ref(0);
const editMode = ref(false);
const draftValues = ref<(string | null)[]>([]);
const pendingSql = ref('');
const executingEdit = ref(false);

const toast = useToast();
const exporting = ref(false);

const EXPORT_ITEMS: MenuItem[] = [
  { key: 'csv', label: '导出为 CSV' },
  { key: 'json', label: '导出为 JSON' },
];

const totalRows = computed(() => data.value?.totalRows ?? 0);
const totalPages = computed(() => Math.max(1, Math.ceil(totalRows.value / pageSize.value)));
const rowOffset = computed(() => (page.value - 1) * pageSize.value);
const canLoad = computed(
  () => Boolean(props.tab.context.connectionId && props.tab.context.tableName),
);

interface DetailItem {
  column: QueryColumn;
  type: string;
  text: string;
}

const detailItems = computed<DetailItem[]>(() => {
  const current = data.value;
  const row = current?.rows[detailIndex.value];
  if (!current || !row) {
    return [];
  }

  return current.columns.map((column, index) => {
    const value = row[index];
    return {
      column,
      type: value?.type ?? 'null',
      text: value?.type === 'null' || value?.value == null ? 'NULL' : value.value,
    };
  });
});

/** 当前行的编辑上下文；缺少主键时为 null */
const editContext = computed<RowEditContext | null>(() => {
  const current = data.value;
  const row = current?.rows[detailIndex.value];
  if (!current || !row) {
    return null;
  }

  return {
    databaseName: props.tab.context.databaseName ?? '',
    tableName: props.tab.context.tableName ?? '',
    columns: current.columns.map((column) => ({
      name: column.name,
      columnType: column.typeName,
    })),
    primaryKey: current.primaryKey,
    original: row,
    next: draftValues.value,
  };
});

const editable = computed(() => (editContext.value ? canEditRow(editContext.value) : false));
const pendingUpdate = computed(() =>
  editContext.value ? buildUpdateSql(editContext.value) : null,
);

async function load(): Promise<void> {
  const { connectionId, databaseName, tableName } = props.tab.context;
  if (!connectionId || !databaseName || !tableName) {
    errorMessage.value = '标签上下文不完整，请重新打开表';
    return;
  }

  // 序列号守卫：并发触发（挂载 + 分页 + 排序 + 筛选）时只让最后一次生效，
  // 避免旧响应迟到覆盖新响应、或 loading 状态被较早完成的请求提前复位
  const seq = ++loadSeq;
  loading.value = true;
  errorMessage.value = '';

  try {
    const result = await getTableData({
      connectionId,
      databaseName,
      tableName,
      page: page.value,
      pageSize: pageSize.value,
      ...(sortColumn.value ? { orderBy: sortColumn.value, orderDesc: sortDesc.value } : {}),
      ...(appliedFilter.value ? { filter: appliedFilter.value } : {}),
    });
    if (seq !== loadSeq) {
      return; // 已有更新的请求发出，丢弃本次过期结果
    }
    data.value = result;
    syncColumns(result.columns);
  } catch (raw) {
    if (seq !== loadSeq) {
      return;
    }
    data.value = null;
    errorMessage.value = normalizeError(raw).message;
  } finally {
    if (seq === loadSeq) {
      loading.value = false;
    }
  }
}

function goPage(next: number): void {
  const target = Math.min(Math.max(1, next), totalPages.value);
  if (target === page.value) {
    return;
  }
  page.value = target;
  void load();
}

function applyFilter(where: string | null): void {
  appliedFilter.value = where ?? '';
  page.value = 1;
  void load();
}

function clearFilter(): void {
  appliedFilter.value = '';
  page.value = 1;
  filterOpen.value = false;
  void load();
}

/** 新增行使用的字段列表（来自表结构，含可空与默认值信息） */
async function openInsert(): Promise<void> {
  const { connectionId, databaseName, tableName } = props.tab.context;
  if (!connectionId || !databaseName || !tableName) {
    toast.error('标签上下文不完整，无法新增行');
    return;
  }

  inserting.value = true;
  try {
    const schema = await getTableSchema(connectionId, databaseName, tableName);
    insertFields.value = schema.columns.map((column) => {
      const isAuto = column.extra.toLowerCase().includes('auto_increment');
      const required = !column.nullable && column.defaultValue === null && !isAuto;
      return {
        column: { name: column.name, columnType: column.columnType },
        mode: required ? 'value' : 'default',
        value: '',
      };
    });
    insertVisible.value = true;
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    inserting.value = false;
  }
}

const insertSql = computed(() =>
  buildInsertSql(
    props.tab.context.databaseName ?? '',
    props.tab.context.tableName ?? '',
    insertFields.value,
  ),
);

function setInsertMode(index: number, mode: 'default' | 'null' | 'value'): void {
  const next = [...insertFields.value];
  const field = next[index];
  if (!field) {
    return;
  }
  next[index] = { ...field, mode };
  insertFields.value = next;
}

function setInsertValue(index: number, value: string): void {
  const next = [...insertFields.value];
  const field = next[index];
  if (!field) {
    return;
  }
  next[index] = { ...field, value, mode: 'value' };
  insertFields.value = next;
}

function requestInsert(): void {
  if (!insertSql.value) {
    toast.warning('至少需要为字段提供一个值');
    return;
  }
  pendingSql.value = insertSql.value;
}

function handleSort(column: string): void {
  if (sortColumn.value === column) {
    sortDesc.value = !sortDesc.value;
  } else {
    sortColumn.value = column;
    sortDesc.value = false;
  }
  page.value = 1;
  void load();
}

async function handleExport(format: string): Promise<void> {
  const { connectionId, databaseName, tableName } = props.tab.context;
  if (!connectionId || !databaseName || !tableName) {
    toast.error('标签上下文不完整，无法导出');
    return;
  }

  const extension = format === 'json' ? 'json' : 'csv';
  const suffix = appliedFilter.value ? '-filtered' : '';

  let path: string | null = null;
  try {
    path = await save({
      title: '导出表数据',
      defaultPath: `${tableName}${suffix}.${extension}`,
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
    const reply = await exportTableData({
      connectionId,
      databaseName,
      tableName,
      format: extension,
      path,
      ...(sortColumn.value ? { orderBy: sortColumn.value, orderDesc: sortDesc.value } : {}),
      ...(appliedFilter.value ? { filter: appliedFilter.value } : {}),
    });

    toast.success(
      reply.truncated
        ? `已导出 ${reply.rowsWritten} 行（达到上限，已截断）`
        : `已导出 ${reply.rowsWritten} 行到 ${reply.path}`,
    );
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    exporting.value = false;
  }
}

function openDetail(index: number): void {
  const row = data.value?.rows[index];
  detailIndex.value = index;
  draftValues.value = row ? row.map((cell) => (cell.type === 'null' ? null : (cell.value ?? ''))) : [];
  editMode.value = false;
  pendingSql.value = '';
  detailVisible.value = true;
}

function setDraft(index: number, value: string): void {
  const next = [...draftValues.value];
  next[index] = value;
  draftValues.value = next;
}

function toggleNull(index: number): void {
  const next = [...draftValues.value];
  next[index] = next[index] === null ? '' : null;
  draftValues.value = next;
}

function startEdit(): void {
  if (!editable.value) {
    toast.warning('该表没有可用主键，无法编辑');
    return;
  }
  editMode.value = true;
}

/** 把当前修改加入待保存队列（不立即执行） */
function queueUpdate(): void {
  const context = editContext.value;
  if (!context) {
    return;
  }

  if (!pendingUpdate.value) {
    toast.info('没有修改内容');
    return;
  }

  const key = rowKeyOf(context.original);
  if (!key) {
    toast.error('缺少可用主键，无法保存修改');
    return;
  }

  pending.value = {
    ...pending.value,
    [key]: { kind: 'update', original: context.original, next: [...draftValues.value] },
  };

  detailVisible.value = false;
  toast.success('已加入待保存变更，点「执行变更」统一提交');
}

function queueDelete(): void {
  const context = editContext.value;
  if (!context) {
    return;
  }

  const key = rowKeyOf(context.original);
  if (!key) {
    toast.error('缺少可用主键，无法删除');
    return;
  }

  pending.value = {
    ...pending.value,
    [key]: { kind: 'delete', original: context.original, next: [] },
  };

  detailVisible.value = false;
  toast.info('已标记删除，执行变更后生效');
}

function discardChanges(): void {
  pending.value = {};
  toast.info('已丢弃待保存变更');
}

function openBatchPreview(): void {
  if (batchStatements.value.length === 0) {
    toast.info('没有可执行的变更');
    return;
  }
  batchPendingSql.value = batchStatements.value.join('\n');
  batchVisible.value = true;
}

async function executeBatch(): Promise<void> {
  const { connectionId, databaseName } = props.tab.context;
  if (!connectionId || !batchPendingSql.value) {
    return;
  }

  batchExecuting.value = true;
  try {
    const reply = await executeBatchSql({
      connectionId,
      databaseName,
      sql: batchPendingSql.value,
      sourceType: 'table',
    });

    if (reply.error) {
      toast.error(reply.message);
      // 失败已整体回滚，待保存变更保留，便于修正后重试
      return;
    }

    pending.value = {};
    batchVisible.value = false;
    toast.success(reply.message);
    void load();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    batchExecuting.value = false;
  }
}

async function confirmExecute(): Promise<void> {
  const { connectionId, databaseName } = props.tab.context;
  if (!connectionId || !pendingSql.value) {
    return;
  }

  executingEdit.value = true;
  try {
    const reply = await executeSql({
      connectionId,
      databaseName,
      sql: pendingSql.value,
      sourceType: 'table',
    });

    if (reply.error) {
      toast.error(reply.error.message);
      return;
    }

    toast.success(reply.message);
    detailVisible.value = false;
    insertVisible.value = false;
    pendingSql.value = '';
    void load();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    executingEdit.value = false;
  }
}

watch(pageSize, () => {
  page.value = 1;
  void load();
});

onMounted(() => {
  if (canLoad.value) {
    loadPrefs();
    // 数据优先加载：schema 仅用于编辑校验，延迟补齐，避免首次打开时
    // 两个请求同时争抢连接（PostgreSQL 主池为单连接）导致卡在读取中
    void load();
    void ensureSchema();
  }
});
</script>

<template>
  <section class="table-tab">
    <header class="table-tab__bar">
      <span class="table-tab__title">{{ tab.context.tableName }}</span>
      <span class="table-tab__meta">{{ tab.context.databaseName }}</span>
      <span class="bar-divider" aria-hidden="true"></span>
      <span class="table-tab__meta">{{ totalRows }} 行</span>
      <span v-if="data" class="table-tab__meta">· {{ data.elapsedMs }}ms</span>
      <span class="table-tab__spacer"></span>

      <button
        v-if="pendingCount > 0"
        type="button"
        class="pending-badge"
        title="查看待执行的 SQL"
        @click="openBatchPreview"
      >
        <Save :size="12" />
        待保存 {{ pendingCount }} 项
      </button>
      <AppButton v-if="pendingCount > 0" variant="ghost" size="sm" @click="discardChanges">
        丢弃
      </AppButton>
      <AppButton v-if="pendingCount > 0" variant="primary" size="sm" @click="openBatchPreview">
        执行变更
      </AppButton>

      <ColumnMenu
        :columns="orderedColumns"
        :hidden="hiddenColumns"
        @toggle="toggleColumn"
        @move="moveColumn"
        @reset="resetColumns"
      />

      <AppButton variant="ghost" size="sm" :disabled="loading || inserting" @click="openInsert">
        <Plus :size="13" />
        新增行
      </AppButton>
      <AppDropdown :items="EXPORT_ITEMS" placement="bottom-end" @select="handleExport">
        <template #default="{ toggle }">
          <AppButton variant="ghost" size="sm" :disabled="loading || exporting" @click="toggle">
            <Download :size="13" />
            {{ exporting ? '导出中…' : '导出' }}
          </AppButton>
        </template>
      </AppDropdown>
      <AppButton variant="ghost" size="sm" :disabled="!canLoad" title="查看当前查询语句" @click="showSqlVisible = true">
        <Eye :size="13" />
        查看 SQL
      </AppButton>
      <AppButton variant="ghost" size="sm" :disabled="loading" title="刷新数据" @click="load">
        <RefreshCw :size="13" :class="{ 'is-spinning': loading }" />
        刷新
      </AppButton>
    </header>

    <div class="table-tab__filter">
      <button
        type="button"
        class="table-tab__filter-toggle"
        :class="{ 'is-active': filterOpen || appliedFilter }"
        @click="filterOpen = !filterOpen"
      >
        <Filter :size="12" />
        筛选
      </button>
      <span v-if="appliedFilter" class="table-tab__filter-expr" :title="appliedFilter">
        {{ appliedFilter }}
      </span>
      <span v-else class="table-tab__meta">未设置筛选条件</span>
      <span class="table-tab__spacer"></span>
      <AppButton v-if="appliedFilter" variant="ghost" size="sm" @click="clearFilter">
        清除
      </AppButton>
    </div>

    <FilterPanel
      v-if="filterOpen"
      :columns="
        (data?.columns ?? []).map((column) => ({
          name: column.name,
          columnType: column.typeName,
        }))
      "
      @apply="applyFilter"
      @close="filterOpen = false"
    />

    <div class="table-tab__body" :class="{ 'is-loading': loading }">
      <div v-if="errorMessage" class="table-tab__error">
        <CircleAlert :size="14" />
        <span>{{ errorMessage }}</span>
      </div>

      <ResultGrid
        v-else-if="viewColumns.length > 0"
        :set="{
          columns: viewColumns,
          rows: viewRows,
          rowCount: viewRows.length,
          truncated: false,
        }"
        :row-offset="rowOffset"
        :highlight-rows="highlightRows"
        editable
        :cell-overrides="cellOverrides"
        :column-widths="columnWidths"
        @cell-commit="handleCellCommit"
        @column-resize="handleColumnResize"
        sortable
        resizable
        :sort-column="sortColumn"
        :sort-desc="sortDesc"
        @sort="handleSort"
        @row-click="openDetail"
      />

      <p v-else-if="loading" class="table-tab__hint">正在读取数据…</p>
      <p v-else class="table-tab__hint">没有符合条件的记录</p>
    </div>

    <footer class="table-tab__foot">
      <span class="table-tab__meta">第 {{ page }} / {{ totalPages }} 页</span>
      <span v-if="sortColumn" class="table-tab__meta">
        · 按 {{ sortColumn }} {{ sortDesc ? '降序' : '升序' }}
      </span>
      <span class="table-tab__spacer"></span>

      <label class="table-tab__size">
        每页
        <select v-model.number="pageSize">
          <option v-for="size in pageSizeOptions" :key="size" :value="size">{{ size }}</option>
        </select>
        行
      </label>

      <AppButton
        variant="ghost"
        size="sm"
        :disabled="page <= 1 || loading"
        title="上一页"
        @click="goPage(page - 1)"
      >
        <ChevronLeft :size="13" />
      </AppButton>
      <AppButton
        variant="ghost"
        size="sm"
        :disabled="page >= totalPages || loading"
        title="下一页"
        @click="goPage(page + 1)"
      >
        <ChevronRight :size="13" />
      </AppButton>
    </footer>

    <!-- 批量变更确认 -->
    <AppModal v-model="batchVisible" title="执行待保存变更" :width="640" hide-footer>
      <p class="confirm-hint">
        将在一个事务中执行 {{ batchStatements.length }} 条语句；任何一条失败都会整体回滚，数据保持原样。
      </p>
      <pre class="sql-preview">{{ batchPendingSql }}</pre>
      <div class="modal-actions">
        <AppButton size="sm" @click="batchVisible = false">返回</AppButton>
        <AppButton variant="danger" size="sm" :loading="batchExecuting" @click="executeBatch">
          执行 {{ batchStatements.length }} 条语句
        </AppButton>
      </div>
    </AppModal>

    <AppModal v-model="insertVisible" title="新增行" :width="620" hide-footer>
      <template v-if="pendingSql">
        <p class="confirm-hint">确认后将立即执行以下语句：</p>
        <pre class="sql-preview">{{ pendingSql }}</pre>
        <div class="modal-actions">
          <AppButton size="sm" @click="pendingSql = ''">返回</AppButton>
          <AppButton
            variant="danger"
            size="sm"
            :loading="executingEdit"
            @click="confirmExecute"
          >
            确认执行
          </AppButton>
        </div>
      </template>

      <template v-else>
        <p class="confirm-hint">
          选择「默认值」的列不会出现在语句里（含自增主键），由数据库填充。
        </p>

        <div class="fields">
          <div v-for="(field, index) in insertFields" :key="field.column.name" class="field">
            <label class="field__key">
              {{ field.column.name }}
              <span class="field__type">{{ field.column.columnType }}</span>
            </label>
            <div class="field__control">
              <select
                class="field__mode"
                :value="field.mode"
                @change="
                  setInsertMode(
                    index,
                    ($event.target as HTMLSelectElement).value as 'default' | 'null' | 'value',
                  )
                "
              >
                <option value="default">默认值</option>
                <option value="null">NULL</option>
                <option value="value">指定值</option>
              </select>
              <input
                class="field__input"
                type="text"
                spellcheck="false"
                autocomplete="off"
                :value="field.value"
                :disabled="field.mode !== 'value'"
                placeholder="值"
                @input="setInsertValue(index, ($event.target as HTMLInputElement).value)"
              />
            </div>
          </div>
        </div>

        <p v-if="insertSql" class="pending-sql">待执行：{{ insertSql }}</p>
        <p v-else class="confirm-hint is-warning">至少需要为字段指定一个值。</p>

        <div class="modal-actions">
          <AppButton size="sm" variant="ghost" @click="insertVisible = false">取消</AppButton>
          <AppButton variant="primary" size="sm" :disabled="!insertSql" @click="requestInsert">
            生成并执行
          </AppButton>
        </div>
      </template>
    </AppModal>

    <AppModal
      v-model="detailVisible"
      :title="`第 ${rowOffset + detailIndex + 1} 行`"
      :width="600"
      hide-footer
    >
      <template v-if="editMode">
        <p v-if="!editable" class="confirm-hint is-warning">
          该表没有可用主键，无法安全定位单行，界面编辑已禁用。
        </p>
        <div class="fields">
          <div v-for="(item, index) in detailItems" :key="item.column.name" class="field">
            <label class="field__key">
              {{ item.column.name }}
              <span class="field__type">{{ item.column.typeName }}</span>
            </label>
            <div class="field__control">
              <input
                v-if="draftValues[index] !== null"
                class="field__input"
                type="text"
                spellcheck="false"
                autocomplete="off"
                :value="draftValues[index] ?? ''"
                @input="setDraft(index, ($event.target as HTMLInputElement).value)"
              />
              <span v-else class="field__null">NULL</span>
              <button type="button" class="field__toggle" @click="toggleNull(index)">
                {{ draftValues[index] === null ? '设值' : '置 NULL' }}
              </button>
            </div>
          </div>
        </div>

        <p v-if="pendingUpdate" class="pending-sql">待加入变更：{{ pendingUpdate }}</p>

        <div class="modal-actions">
          <AppButton size="sm" variant="ghost" @click="editMode = false">取消</AppButton>
          <AppButton size="sm" variant="danger" @click="queueDelete">
            <Trash2 :size="12" />
            标记删除
          </AppButton>
          <AppButton variant="primary" size="sm" :disabled="!pendingUpdate" @click="queueUpdate">
            加入待保存
          </AppButton>
        </div>
      </template>

      <template v-else>
        <dl class="detail">
          <div v-for="item in detailItems" :key="item.column.name" class="detail__item">
            <dt class="detail__key">
              {{ item.column.name }}
              <span class="detail__type">{{ item.column.typeName }}</span>
            </dt>
            <dd class="detail__value" :class="{ 'is-null': item.type === 'null' }">
              {{ item.text }}
            </dd>
          </div>
        </dl>

        <div class="modal-actions">
          <AppButton size="sm" @click="detailVisible = false">关闭</AppButton>
          <AppButton
            variant="primary"
            size="sm"
            :disabled="!editable"
            :title="editable ? '编辑这一行' : '该表没有可用主键'"
            @click="startEdit"
          >
            <Pencil :size="12" />
            编辑
          </AppButton>
        </div>
      </template>
    </AppModal>

    <!-- 查看当前查询语句 -->
    <AppModal v-model="showSqlVisible" title="当前查询语句" :width="640" hide-footer>
      <p class="confirm-hint">
        这是当前视图对应执行的 SELECT（含筛选、排序与分页），可复制用于学习或复现。
      </p>
      <pre class="sql-preview">{{ currentSelectSql }}</pre>
      <div class="modal-actions">
        <AppButton size="sm" @click="showSqlVisible = false">关闭</AppButton>
        <AppButton variant="primary" size="sm" @click="copyCurrentSql">复制</AppButton>
      </div>
    </AppModal>
  </section>
</template>

<style scoped>
.table-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--surface-bg);
}

.table-tab__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.table-tab__title {
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 500;
}

.table-tab__meta {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.bar-divider {
  width: 1px;
  height: 10px;
  background-color: var(--border-color);
}

.table-tab__spacer {
  flex: 1;
}

.pending-badge {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  height: 22px;
  padding: 0 8px;
  color: var(--warning-color);
  font-size: 10px;
  background-color: var(--warning-soft);
  border-radius: var(--radius-xs);
}

.table-tab__filter {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--surface-subtle);
}

.table-tab__filter-toggle {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 8px;
  color: var(--text-secondary);
  font-size: 11px;
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.table-tab__filter-toggle:hover {
  color: var(--text-primary);
}

.table-tab__filter-toggle.is-active {
  color: var(--primary-color);
  background-color: var(--primary-soft);
  border-color: var(--primary-color);
}

.table-tab__filter-expr {
  overflow: hidden;
  max-width: 60%;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.table-tab__body {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.table-tab__body :deep(.grid) {
  height: 100%;
}

.table-tab__body.is-loading {
  cursor: progress;
}

.table-tab__body.is-loading :deep(.grid) {
  opacity: 0.55;
}

.table-tab__error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: var(--space-3);
  color: var(--danger-color);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  user-select: text;
}

.table-tab__hint {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.table-tab__foot {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 32px;
  padding: 0 var(--space-3);
  border-top: 1px solid var(--divider-color);
}

.table-tab__size {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--text-tertiary);
  font-size: 11px;
}

.table-tab__size select {
  height: 22px;
  padding: 0 4px;
  color: var(--text-primary);
  font-size: 11px;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-xs);
  outline: none;
}

.modal-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  margin-top: var(--space-4);
  padding-top: var(--space-3);
  border-top: 1px solid var(--divider-color);
}

.confirm-hint {
  margin-bottom: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  line-height: 1.7;
}

.confirm-hint.is-warning {
  color: var(--warning-color);
}

.sql-preview,
.pending-sql {
  padding: var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  user-select: text;
}

.pending-sql {
  max-height: 80px;
  margin-top: var(--space-3);
  overflow: auto;
  color: var(--text-tertiary);
  font-size: 10px;
}

.fields {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  max-height: 50vh;
  overflow: auto;
}

.field {
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: var(--space-3);
  align-items: center;
  padding: 4px var(--space-2);
  border-bottom: 1px solid var(--divider-color);
}

.field__key {
  display: flex;
  flex-direction: column;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  word-break: break-all;
}

.field__type {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
}

.field__control {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-width: 0;
}

.field__mode {
  flex: none;
  height: 26px;
  padding: 0 4px;
  color: var(--text-secondary);
  font-size: 11px;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.field__input {
  flex: 1;
  min-width: 0;
  height: 26px;
  padding: 0 var(--space-2);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.field__input:focus {
  border-color: var(--primary-color);
}

.field__null {
  flex: 1;
  padding: 0 var(--space-2);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  font-style: italic;
  line-height: 26px;
  background-color: var(--surface-subtle);
  border: 1px dashed var(--border-color);
  border-radius: var(--radius-sm);
}

.field__toggle {
  flex: none;
  padding: 2px 6px;
  color: var(--text-tertiary);
  font-size: 10px;
  border-radius: var(--radius-xs);
}

.field__toggle:hover {
  color: var(--primary-color);
  background-color: var(--surface-hover);
}

.detail {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.detail__item {
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: var(--space-3);
  padding: 6px var(--space-2);
  border-bottom: 1px solid var(--divider-color);
}

.detail__item:last-child {
  border-bottom: none;
}

.detail__key {
  display: flex;
  flex-direction: column;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  word-break: break-all;
}

.detail__type {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
}

.detail__value {
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
}

.detail__value.is-null {
  color: var(--text-tertiary);
  font-style: italic;
}

.is-spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .is-spinning {
    animation: none;
  }
}
</style>
