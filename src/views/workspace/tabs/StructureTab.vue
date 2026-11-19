<script setup lang="ts">
import {
  Check,
  CircleAlert,
  Copy,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Trash2,
} from 'lucide-vue-next';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppCombobox from '@/components/common/AppCombobox.vue';
import AppModal from '@/components/common/AppModal.vue';

import { executeSql } from '@/api/query';
import { getTableSchema } from '@/api/schema';
import { useToast } from '@/composables/useToast';
import { useUiStore } from '@/stores/ui';
import { copyText } from '@/utils/clipboard';
import { normalizeError } from '@/utils/errors';
import {
  buildSchemaChanges,
  composeColumnType,
  COLUMN_TYPE_GROUPS,
  parseColumnType,
  toColumnDrafts,
  toIndexDrafts,
  typeNeedsLength,
  validateDrafts,
} from '@/utils/table-ddl';

import type { TableSchema } from '@/types/database';
import type { ColumnDraft, IndexDraft } from '@/utils/table-ddl';
import type { WorkspaceTab } from '@/types/workspace';

/**
 * 表结构标签：字段 / 索引可视化设计器 + 可直接执行的 DDL 视图。
 *
 * - 字段、索引可在表格内直接编辑，保存时对比原结构生成最小 ALTER 集合；
 * - `Ctrl/⌘ + D` 在「字段」与「DDL」视图间切换（与 Navicat 习惯一致）；
 * - 执行前展示将要发送的完整 SQL，DDL 不可回滚，必须确认。
 */
const props = defineProps<{ tab: WorkspaceTab }>();

const toast = useToast();
const uiStore = useUiStore();

type StructureView = 'fields' | 'ddl';

const loading = ref(false);
const executing = ref(false);
const errorMessage = ref('');
const schema = ref<TableSchema | null>(null);
const copied = ref(false);

const view = ref<StructureView>('fields');
const isMac = navigator.userAgent.includes('Mac');
const shortcutLabel = isMac ? '⌘D' : 'Ctrl+D';

/** 原始结构（用于计算差异） */
const originalColumns = ref<ColumnDraft[]>([]);
const originalIndexes = ref<IndexDraft[]>([]);
const originalPrimaryKey = ref<string[]>([]);

/** 编辑中的草稿 */
const columns = ref<ColumnDraft[]>([]);
const indexes = ref<IndexDraft[]>([]);

/** DDL 视图 */
const ddlDraft = ref('');

/** 执行确认 */
const confirmVisible = ref(false);
const confirmTitle = ref('');
const pendingSql = ref('');

const databaseName = computed(() => props.tab.context.databaseName ?? '');
const tableName = computed(() => props.tab.context.tableName ?? '');

/** 字段类型下拉选项：分组类型 + 当前草稿里出现的非标准类型（避免 select 丢值） */
const allTypeBases = computed<string[]>(() => {
  const bases = COLUMN_TYPE_GROUPS.flatMap((group) => group.types);
  const extras = new Set<string>();
  for (const column of columns.value) {
    const base = parseColumnType(column.columnType).base.toUpperCase();
    if (base && !bases.some((item) => item.toLowerCase() === base.toLowerCase())) {
      extras.add(base);
    }
  }
  return [...bases, ...extras];
});

const statements = computed(() =>
  buildSchemaChanges({
    database: databaseName.value,
    table: tableName.value,
    originalColumns: originalColumns.value,
    originalIndexes: originalIndexes.value,
    originalPrimaryKey: originalPrimaryKey.value,
    columns: columns.value,
    indexes: indexes.value,
  }),
);

const dirty = computed(() => statements.value.length > 0);
const ddlDirty = computed(
  () => schema.value !== null && ddlDraft.value.trim() !== schema.value.ddl.trim(),
);

async function load(): Promise<void> {
  const { connectionId, databaseName: database, tableName: table } = props.tab.context;
  if (!connectionId || !database || !table) {
    errorMessage.value = '标签上下文不完整，请重新打开表';
    return;
  }

  loading.value = true;
  errorMessage.value = '';

  try {
    const data = await getTableSchema(connectionId, database, table);
    schema.value = data;
    originalColumns.value = toColumnDrafts(data);
    originalIndexes.value = toIndexDrafts(data);
    originalPrimaryKey.value = data.columns
      .filter((column) => column.columnKey.toUpperCase() === 'PRI')
      .map((column) => column.name);
    columns.value = toColumnDrafts(data);
    indexes.value = toIndexDrafts(data);
    ddlDraft.value = data.ddl;
  } catch (raw) {
    schema.value = null;
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

/* ---------- 字段编辑 ---------- */

function updateColumn(index: number, patch: Partial<ColumnDraft>): void {
  const next = [...columns.value];
  const current = next[index];
  if (!current) {
    return;
  }
  next[index] = { ...current, ...patch };
  columns.value = next;
}

function addColumn(): void {
  columns.value = [
    ...columns.value,
    {
      key: `new-${Date.now()}-${columns.value.length}`,
      originalName: null,
      name: '',
      columnType: 'varchar(255)',
      nullable: true,
      defaultValue: null,
      comment: '',
      primary: false,
      autoIncrement: false,
      onUpdate: null,
    },
  ];
}

function removeColumn(index: number): void {
  const target = columns.value[index];
  if (!target) {
    return;
  }
  columns.value = columns.value.filter((_, position) => position !== index);
}

/* ---------- 索引编辑 ---------- */

function updateIndex(index: number, patch: Partial<IndexDraft>): void {
  const next = [...indexes.value];
  const current = next[index];
  if (!current) {
    return;
  }
  next[index] = { ...current, ...patch };
  indexes.value = next;
}

function addIndex(): void {
  indexes.value = [
    ...indexes.value,
    {
      key: `new-idx-${Date.now()}-${indexes.value.length}`,
      originalName: null,
      name: '',
      unique: false,
      columns: '',
    },
  ];
}

function removeIndex(index: number): void {
  indexes.value = indexes.value.filter((_, position) => position !== index);
}

/** 当前表内全部字段名（供索引勾选） */
const columnNameList = computed<string[]>(() =>
  columns.value
    .map((column) => column.name.trim())
    .filter((name) => name.length > 0),
);

/** 解析某个索引草稿已勾选的列集合 */
function indexColumnList(draftColumns: string): string[] {
  return draftColumns
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

/** 从索引中移除一列，返回新的逗号分隔串 */
function removeIndexColumn(current: string, name: string): string {
  return indexColumnList(current)
    .filter((item) => item !== name)
    .join(', ');
}

/** 向索引添加一列（去重），返回新的逗号分隔串 */
function addIndexColumn(current: string, name: string): string {
  const list = indexColumnList(current);
  if (list.includes(name)) {
    return list.join(', ');
  }
  return [...list, name].join(', ');
}

/** 交换索引中两列的顺序（用于上下移动），返回新的逗号分隔串 */
function moveIndexColumn(current: string, from: number, to: number): string {
  const list = indexColumnList(current);
  if (from < 0 || from >= list.length || to < 0 || to >= list.length) {
    return list.join(', ');
  }
  const next = [...list];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next.join(', ');
}

function discard(): void {
  if (!schema.value) {
    return;
  }
  columns.value = toColumnDrafts(schema.value);
  indexes.value = toIndexDrafts(schema.value);
  toast.info('已恢复为当前表结构');
}

/* ---------- 执行 ---------- */

function requestSave(): void {
  const error = validateDrafts(columns.value, indexes.value);
  if (error) {
    toast.warning(error);
    return;
  }
  if (statements.value.length === 0) {
    toast.info('没有需要保存的修改');
    return;
  }

  confirmTitle.value = `执行 ${statements.value.length} 条结构变更`;
  pendingSql.value = statements.value.join('\n');
  confirmVisible.value = true;
}

async function executeDdl(): Promise<void> {
  if (ddlDraft.value.trim().length === 0) {
    toast.warning('DDL 内容为空');
    return;
  }
  confirmTitle.value = '执行 DDL';
  pendingSql.value = ddlDraft.value;
  confirmVisible.value = true;
}

async function confirmExecute(): Promise<void> {
  const { connectionId, databaseName: database } = props.tab.context;
  if (!connectionId || !pendingSql.value) {
    return;
  }

  executing.value = true;
  try {
    const reply = await executeSql({
      connectionId,
      databaseName: database,
      sql: pendingSql.value,
      sourceType: 'structure',
    });

    if (reply.error) {
      toast.error(`第 ${reply.error.index} 条语句失败：${reply.error.message}`);
      return;
    }

    confirmVisible.value = false;
    toast.success(reply.message);
    uiStore.bumpHistory();
    await load();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    executing.value = false;
  }
}

/* ---------- 复制 ---------- */

async function copyDdl(): Promise<void> {
  const ddl = view.value === 'ddl' ? ddlDraft.value : (schema.value?.ddl ?? '');
  if (!ddl) {
    return;
  }

  if (await copyText(ddl)) {
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 1500);
    return;
  }

  errorMessage.value = '复制失败，请手动选择文本复制';
}

/* ---------- 快捷键 ---------- */

function handleKeydown(event: KeyboardEvent): void {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'd') {
    event.preventDefault();
    view.value = view.value === 'fields' ? 'ddl' : 'fields';
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
  void load();
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <section class="structure-tab">
    <header class="structure-tab__bar">
      <span class="structure-tab__title">{{ tableName }}</span>
      <span class="structure-tab__meta">{{ databaseName }}</span>
      <span class="bar-divider" aria-hidden="true"></span>

      <div class="view-switch">
        <button
          type="button"
          class="view-switch__item"
          :class="{ 'is-active': view === 'fields' }"
          @click="view = 'fields'"
        >
          字段与索引
        </button>
        <button
          type="button"
          class="view-switch__item"
          :class="{ 'is-active': view === 'ddl' }"
          :title="`切换到 DDL（${shortcutLabel}）`"
          @click="view = 'ddl'"
        >
          DDL
        </button>
      </div>

      <span v-if="schema" class="structure-tab__meta">
        {{ columns.length }} 字段 · {{ indexes.length }} 索引
      </span>
      <span v-if="dirty" class="dirty-badge">有未保存修改</span>

      <span class="structure-tab__spacer"></span>

      <AppButton variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="13" />
        刷新
      </AppButton>
      <AppButton variant="ghost" size="sm" :disabled="!schema" @click="copyDdl">
        <Check v-if="copied" :size="13" />
        <Copy v-else :size="13" />
        {{ copied ? '已复制' : '复制' }}
      </AppButton>
      <AppButton
        v-if="view === 'fields'"
        variant="ghost"
        size="sm"
        :disabled="!dirty"
        @click="discard"
      >
        <RotateCcw :size="13" />
        撤销
      </AppButton>
      <AppButton
        v-if="view === 'fields'"
        variant="primary"
        size="sm"
        :disabled="loading || !dirty"
        @click="requestSave"
      >
        <Save :size="13" />
        保存变更
      </AppButton>
      <AppButton
        v-else
        variant="primary"
        size="sm"
        :disabled="loading || !ddlDirty"
        @click="executeDdl"
      >
        执行 DDL
      </AppButton>
    </header>

    <div class="structure-tab__body">
      <div v-if="errorMessage" class="structure-tab__error">
        <CircleAlert :size="14" />
        <span>{{ errorMessage }}</span>
      </div>

      <p v-else-if="loading && !schema" class="structure-tab__hint">正在读取表结构…</p>

      <!-- 字段与索引设计器 -->
      <template v-else-if="schema && view === 'fields'">
        <section class="block">
          <h3 class="block__title">
            字段
            <span class="block__note">主键列勾选「PK」；新增字段插在上一行之后</span>
            <span class="block__spacer"></span>
            <button type="button" class="link-button" @click="addColumn">
              <Plus :size="12" />
              添加字段
            </button>
          </h3>

          <div class="block__scroll">
            <table class="edit-table">
              <thead>
                <tr>
                  <th class="col-index">#</th>
                  <th class="col-name">名称</th>
                  <th class="col-type">类型</th>
                  <th class="col-null">可空</th>
                  <th class="col-default">默认值</th>
                  <th class="col-pk">PK</th>
                  <th class="col-ai">自增</th>
                  <th class="col-comment">注释</th>
                  <th class="col-action"></th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(column, index) in columns"
                  :key="column.key"
                  :class="{ 'is-new': column.originalName === null }"
                >
                  <td class="col-index is-muted">{{ index + 1 }}</td>
                  <td>
                    <input
                      class="cell-input is-mono"
                      :value="column.name"
                      spellcheck="false"
                      autocomplete="off"
                      placeholder="字段名"
                      @input="
                        updateColumn(index, {
                          name: ($event.target as HTMLInputElement).value,
                        })
                      "
                    />
                  </td>
                  <td>
                    <div class="type-cell">
                      <AppCombobox
                        class="type-select"
                        :model-value="parseColumnType(column.columnType).base"
                        :options="allTypeBases.map((item) => item.toLowerCase())"
                        placeholder="类型"
                        @update:model-value="
                          updateColumn(index, {
                            columnType: composeColumnType(
                              $event,
                              parseColumnType(column.columnType).args,
                            ),
                          })
                        "
                      />
                      <input
                        class="cell-input is-mono type-length"
                        :value="parseColumnType(column.columnType).args ?? ''"
                        :disabled="!typeNeedsLength(parseColumnType(column.columnType).base)"
                        placeholder="长度"
                        title="长度/精度，如 255 或 12,2"
                        spellcheck="false"
                        autocomplete="off"
                        @input="
                          updateColumn(index, {
                            columnType: composeColumnType(
                              parseColumnType(column.columnType).base,
                              ($event.target as HTMLInputElement).value,
                            ),
                          })
                        "
                      />
                    </div>
                  </td>
                  <td class="is-center">
                    <input
                      type="checkbox"
                      :checked="column.nullable"
                      @change="
                        updateColumn(index, {
                          nullable: ($event.target as HTMLInputElement).checked,
                        })
                      "
                    />
                  </td>
                  <td>
                    <input
                      class="cell-input is-mono"
                      :value="column.defaultValue ?? ''"
                      spellcheck="false"
                      autocomplete="off"
                      placeholder="无"
                      @input="
                        updateColumn(index, {
                          defaultValue:
                            ($event.target as HTMLInputElement).value.trim().length > 0
                              ? ($event.target as HTMLInputElement).value
                              : null,
                        })
                      "
                    />
                  </td>
                  <td class="is-center">
                    <input
                      type="checkbox"
                      :checked="column.primary"
                      @change="
                        updateColumn(index, {
                          primary: ($event.target as HTMLInputElement).checked,
                        })
                      "
                    />
                  </td>
                  <td class="is-center">
                    <input
                      type="checkbox"
                      :checked="column.autoIncrement"
                      @change="
                        updateColumn(index, {
                          autoIncrement: ($event.target as HTMLInputElement).checked,
                        })
                      "
                    />
                  </td>
                  <td>
                    <input
                      class="cell-input"
                      :value="column.comment"
                      spellcheck="false"
                      autocomplete="off"
                      placeholder="可选"
                      @input="
                        updateColumn(index, {
                          comment: ($event.target as HTMLInputElement).value,
                        })
                      "
                    />
                  </td>
                  <td class="col-action">
                    <button
                      type="button"
                      class="icon-button"
                      title="删除字段"
                      @click="removeColumn(index)"
                    >
                      <Trash2 :size="12" />
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <section class="block">
          <h3 class="block__title">
            索引
            <span class="block__note">主键请在上方用「PK」勾选；列名用逗号分隔</span>
            <span class="block__spacer"></span>
            <button type="button" class="link-button" @click="addIndex">
              <Plus :size="12" />
              添加索引
            </button>
          </h3>

          <p v-if="indexes.length === 0" class="block__empty">该表没有普通索引</p>
          <div v-else class="block__scroll">
            <table class="edit-table">
              <thead>
                <tr>
                  <th class="col-name">名称</th>
                  <th class="col-unique">唯一</th>
                  <th>列（逗号分隔）</th>
                  <th class="col-action"></th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(index, position) in indexes"
                  :key="index.key"
                  :class="{ 'is-new': index.originalName === null }"
                >
                  <td>
                    <input
                      class="cell-input is-mono"
                      :value="index.name"
                      spellcheck="false"
                      autocomplete="off"
                      placeholder="idx_name"
                      @input="
                        updateIndex(position, {
                          name: ($event.target as HTMLInputElement).value,
                        })
                      "
                    />
                  </td>
                  <td class="is-center">
                    <input
                      type="checkbox"
                      :checked="index.unique"
                      @change="
                        updateIndex(position, {
                          unique: ($event.target as HTMLInputElement).checked,
                        })
                      "
                    />
                  </td>
                  <td>
                    <div class="index-cols">
                      <span v-if="columnNameList.length === 0" class="index-cols__empty">
                        请先添加字段
                      </span>
                      <template v-else>
                        <div class="index-cols__chips">
                          <span
                            v-for="(colName, colIdx) in indexColumnList(index.columns)"
                            :key="colName"
                            class="index-chip"
                          >
                            <span class="index-chip__order">{{ colIdx + 1 }}</span>
                            <span class="index-chip__name">{{ colName }}</span>
                            <button
                              type="button"
                              class="index-chip__btn"
                              title="上移"
                              :disabled="colIdx === 0"
                              @click="
                                updateIndex(position, {
                                  columns: moveIndexColumn(
                                    index.columns,
                                    colIdx,
                                    colIdx - 1,
                                  ),
                                })
                              "
                            >
                              ↑
                            </button>
                            <button
                              type="button"
                              class="index-chip__btn"
                              title="下移"
                              :disabled="colIdx === indexColumnList(index.columns).length - 1"
                              @click="
                                updateIndex(position, {
                                  columns: moveIndexColumn(
                                    index.columns,
                                    colIdx,
                                    colIdx + 1,
                                  ),
                                })
                              "
                            >
                              ↓
                            </button>
                            <button
                              type="button"
                              class="index-chip__btn index-chip__btn--remove"
                              title="移除"
                              @click="
                                updateIndex(position, {
                                  columns: removeIndexColumn(index.columns, colName),
                                })
                              "
                            >
                              ×
                            </button>
                          </span>
                        </div>
                        <AppCombobox
                          class="index-cols__add"
                          :model-value="''"
                          :options="columnNameList"
                          placeholder="添加列（可输入或选择）"
                          @select="
                            updateIndex(position, {
                              columns: addIndexColumn(index.columns, $event),
                            })
                          "
                        />
                      </template>
                    </div>
                  </td>
                  <td class="col-action">
                    <button
                      type="button"
                      class="icon-button"
                      title="删除索引"
                      @click="removeIndex(position)"
                    >
                      <Trash2 :size="12" />
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <section v-if="dirty" class="block">
          <h3 class="block__title">待执行的变更</h3>
          <pre class="ddl">{{ statements.join('\n') }}</pre>
        </section>
      </template>

      <!-- DDL 视图 -->
      <div v-else-if="schema" class="ddl-view">
        <p class="ddl-view__hint">
          可直接修改下面的语句并执行（DDL 不能回滚）。按 {{ shortcutLabel }} 返回字段视图。
        </p>
        <textarea v-model="ddlDraft" class="ddl-editor" spellcheck="false"></textarea>
      </div>
    </div>

    <AppModal v-model="confirmVisible" :title="confirmTitle" :width="700" hide-footer>
      <p class="confirm-hint">
        以下语句将立即在数据库中执行。DDL 无法回滚，请确认无误。
      </p>
      <pre class="ddl">{{ pendingSql }}</pre>
      <div class="modal-actions">
        <AppButton size="sm" @click="confirmVisible = false">返回</AppButton>
        <AppButton variant="danger" size="sm" :loading="executing" @click="confirmExecute">
          确认执行
        </AppButton>
      </div>
    </AppModal>
  </section>
</template>

<style scoped>
.structure-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--surface-bg);
}

.structure-tab__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.structure-tab__title {
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 500;
}

.structure-tab__meta {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.bar-divider {
  width: 1px;
  height: 10px;
  background-color: var(--border-color);
}

.structure-tab__spacer {
  flex: 1;
}

.view-switch {
  display: inline-flex;
  flex: none;
  gap: 2px;
  padding: 1px;
  background-color: var(--surface-subtle);
  border-radius: var(--radius-sm);
}

.view-switch__item {
  height: 22px;
  padding: 0 8px;
  color: var(--text-tertiary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.view-switch__item:hover {
  color: var(--text-primary);
}

.view-switch__item.is-active {
  color: var(--text-primary);
  background-color: var(--surface-bg);
  box-shadow: 0 1px 2px rgb(0 0 0 / 8%);
}

.dirty-badge {
  flex: none;
  padding: 1px 6px;
  color: var(--warning-color);
  font-size: 10px;
  background-color: rgb(217 119 6 / 12%);
  border-radius: var(--radius-xs);
}

.structure-tab__body {
  flex: 1;
  min-height: 0;
  padding: var(--space-3) var(--space-4) var(--space-6);
  overflow: auto;
}

.structure-tab__error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  color: var(--danger-color);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  user-select: text;
}

.structure-tab__hint {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.block + .block {
  margin-top: var(--space-5);
}

.block__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  font-weight: 500;
}

.block__note {
  color: var(--text-tertiary);
  font-weight: 400;
}

.block__spacer {
  flex: 1;
}

.block__empty {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.block__scroll {
  overflow: auto;
  border: 1px solid var(--divider-color);
  border-radius: var(--radius-sm);
}

.edit-table {
  width: 100%;
  font-size: var(--font-size-xs);
  border-collapse: collapse;
}

.edit-table th,
.edit-table td {
  padding: 3px var(--space-2);
  text-align: left;
  white-space: nowrap;
  border-bottom: 1px solid var(--divider-color);
}

.edit-table thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  color: var(--text-secondary);
  font-weight: 500;
  background-color: var(--surface-subtle);
}

.edit-table tbody tr:hover td {
  background-color: var(--surface-hover);
}

.edit-table tbody tr.is-new td {
  background-color: rgb(34 197 94 / 8%);
}

.col-index {
  width: 34px;
}

.col-name {
  width: 180px;
}

.col-type {
  width: 250px;
}

.type-cell {
  display: flex;
  align-items: center;
  gap: 4px;
}

.type-select {
  flex: none;
  width: 130px;
}

.type-length {
  flex: 1;
  min-width: 0;
}

.type-length:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.index-cols {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 24px;
  padding: 3px 0;
}

.index-cols__empty {
  color: var(--text-tertiary);
  font-size: 10px;
}

.index-cols__chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.index-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  height: 22px;
  padding: 0 4px 0 6px;
  color: var(--text-secondary);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: 10px;
  user-select: none;
}

.index-chip__order {
  flex: none;
  padding: 0 4px;
  color: var(--text-inverse);
  background-color: var(--primary-color);
  border-radius: 999px;
  font-size: 9px;
  line-height: 14px;
}

.index-chip__name {
  color: var(--text-primary);
}

.index-chip__btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
  font-size: 11px;
  line-height: 1;
}

.index-chip__btn:hover:not(:disabled) {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.index-chip__btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.index-chip__btn--remove:hover:not(:disabled) {
  color: var(--danger-color);
  background-color: var(--danger-soft);
}

.index-cols__add {
  width: 200px;
}

.col-null,
.col-pk,
.col-ai,
.col-unique {
  width: 46px;
  text-align: center;
}

.col-default {
  width: 150px;
}

.col-comment {
  min-width: 140px;
}

.col-action {
  width: 40px;
  text-align: center;
}

.cell-input {
  width: 100%;
  min-width: 0;
  height: 24px;
  padding: 0 6px;
  color: var(--text-primary);
  font-family: inherit;
  font-size: var(--font-size-xs);
  background-color: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-xs);
  outline: none;
}

.cell-input:hover {
  border-color: var(--border-color);
}

.cell-input:focus {
  background-color: var(--surface-bg);
  border-color: var(--primary-color);
}

.cell-input::placeholder {
  color: var(--text-tertiary);
}

.is-mono {
  font-family: var(--font-mono);
}

.is-muted {
  color: var(--text-tertiary);
}

.is-center {
  text-align: center;
}

.link-button {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--primary-color);
  font-size: 11px;
}

.link-button:hover {
  text-decoration: underline;
}

.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
}

.icon-button:hover {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}

.ddl-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  height: 100%;
}

.ddl-view__hint {
  color: var(--text-tertiary);
  font-size: 11px;
}

.ddl-editor {
  flex: 1;
  min-height: 320px;
  padding: var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.75;
  white-space: pre;
  background-color: var(--surface-subtle);
  border: 1px solid var(--divider-color);
  border-radius: var(--radius-sm);
  outline: none;
  resize: vertical;
}

.ddl-editor:focus {
  border-color: var(--primary-color);
}

.ddl {
  padding: var(--space-3);
  overflow: auto;
  max-height: 40vh;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  background-color: var(--surface-subtle);
  border: 1px solid var(--divider-color);
  border-radius: var(--radius-sm);
  user-select: text;
}

.confirm-hint {
  margin-bottom: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  line-height: 1.7;
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
</style>
