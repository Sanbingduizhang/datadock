<script setup lang="ts">
import { Plus, Trash2, X } from 'lucide-vue-next';
import { computed, ref } from 'vue';

import AppButton from '@/components/common/AppButton.vue';

import { buildWhereClause, FILTER_OPERATORS } from '@/utils/table-edit';

import type { EditColumn, FilterCondition, FilterOperator } from '@/utils/table-edit';

/**
 * 表数据筛选面板（Navicat 式条件行）。
 *
 * 每行是「字段 + 运算符 + 值」，行之间用 AND/OR 连接；
 * 需要写复杂表达式时可切到「直接输入」，两种模式互斥。
 */
const props = defineProps<{ columns: EditColumn[] }>();

const emit = defineEmits<{
  apply: [where: string | null];
  close: [];
}>();

interface FilterRow {
  id: number;
  columnIndex: number;
  operator: FilterOperator;
  value: string;
  connector: 'AND' | 'OR';
}

let rowSequence = 0;

const rows = ref<FilterRow[]>([]);
const advanced = ref('');
const advancedMode = ref(false);

function createRow(): FilterRow {
  rowSequence += 1;
  return {
    id: rowSequence,
    columnIndex: 0,
    operator: 'eq',
    value: '',
    connector: 'AND',
  };
}

rows.value = [createRow()];

const conditions = computed<FilterCondition[]>(() =>
  rows.value.map((row) => {
    const column = props.columns[row.columnIndex] ?? props.columns[0];
    return {
      column: column?.name ?? '',
      columnType: column?.columnType ?? '',
      operator: row.operator,
      value: row.value,
      connector: row.connector,
    };
  }),
);

const preview = computed(() =>
  advancedMode.value ? advanced.value.trim() || null : buildWhereClause(conditions.value),
);

function needsValue(operator: FilterOperator): boolean {
  return FILTER_OPERATORS.find((item) => item.value === operator)?.needValue ?? true;
}

function addRow(): void {
  rows.value = [...rows.value, createRow()];
}

function removeRow(id: number): void {
  rows.value = rows.value.filter((row) => row.id !== id);
  if (rows.value.length === 0) {
    rows.value = [createRow()];
  }
}

function apply(): void {
  emit('apply', preview.value);
  emit('close');
}

function clear(): void {
  rows.value = [createRow()];
  advanced.value = '';
  emit('apply', null);
  emit('close');
}
</script>

<template>
  <div class="filter">
    <div class="filter__rows">
      <div v-for="(row, index) in rows" :key="row.id" class="filter__row">
        <select
          v-if="index > 0"
          v-model="row.connector"
          class="filter__control filter__control--connector"
          title="与上一行的连接方式"
        >
          <option value="AND">且</option>
          <option value="OR">或</option>
        </select>
        <span v-else class="filter__where">条件</span>

        <select v-model.number="row.columnIndex" class="filter__control filter__control--column">
          <option v-for="(column, columnIndex) in columns" :key="column.name" :value="columnIndex">
            {{ column.name }}
          </option>
        </select>

        <select v-model="row.operator" class="filter__control filter__control--operator">
          <option v-for="operator in FILTER_OPERATORS" :key="operator.value" :value="operator.value">
            {{ operator.label }}
          </option>
        </select>

        <input
          v-model="row.value"
          class="filter__control filter__control--value"
          type="text"
          spellcheck="false"
          autocomplete="off"
          :disabled="!needsValue(row.operator)"
          :placeholder="needsValue(row.operator) ? '值' : '无需填写'"
          @keydown.enter="apply"
        />

        <button type="button" class="filter__remove" title="删除这行条件" @click="removeRow(row.id)">
          <Trash2 :size="12" />
        </button>
      </div>

      <div class="filter__actions">
        <button type="button" class="filter__add" @click="addRow">
          <Plus :size="12" />
          添加条件
        </button>
        <button type="button" class="filter__mode" @click="advancedMode = !advancedMode">
          {{ advancedMode ? '使用条件行' : '直接输入条件' }}
        </button>
      </div>
    </div>

    <textarea
      v-if="advancedMode"
      v-model="advanced"
      class="filter__advanced"
      spellcheck="false"
      placeholder="WHERE 之后的内容，例如 status = 1 AND created_at >= '2026-01-01'"
    ></textarea>

    <footer class="filter__foot">
      <span class="filter__preview" :title="preview ?? ''">
        {{ preview ? `WHERE ${preview}` : '未设置条件' }}
      </span>
      <span class="filter__spacer"></span>
      <AppButton variant="ghost" size="sm" @click="clear">清除</AppButton>
      <AppButton variant="primary" size="sm" @click="apply">应用筛选</AppButton>
      <button type="button" class="filter__close" title="收起" @click="emit('close')">
        <X :size="13" />
      </button>
    </footer>
  </div>
</template>

<style scoped>
.filter {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background-color: var(--surface-subtle);
  border-bottom: 1px solid var(--divider-color);
}

.filter__rows {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 180px;
  overflow: auto;
}

.filter__row {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.filter__where,
.filter__control--connector {
  flex: none;
  width: 52px;
  color: var(--text-tertiary);
  font-size: 11px;
  text-align: center;
}

.filter__control {
  height: 24px;
  padding: 0 4px;
  color: var(--text-primary);
  font-size: 11px;
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-xs);
  outline: none;
}

.filter__control:focus {
  border-color: var(--primary-color);
}

.filter__control--column {
  width: 130px;
}

.filter__control--operator {
  width: 140px;
}

.filter__control--value {
  flex: 1;
  min-width: 80px;
  font-family: var(--font-mono);
}

.filter__control--value:disabled {
  color: var(--text-tertiary);
  background-color: var(--surface-hover);
}

.filter__remove {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
}

.filter__remove:hover {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}

.filter__actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding-left: 54px;
}

.filter__add,
.filter__mode {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--primary-color);
  font-size: 11px;
}

.filter__mode {
  color: var(--text-tertiary);
}

.filter__mode:hover,
.filter__add:hover {
  text-decoration: underline;
}

.filter__advanced {
  width: 100%;
  height: 54px;
  padding: var(--space-2);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.6;
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
  resize: vertical;
}

.filter__advanced:focus {
  border-color: var(--primary-color);
}

.filter__foot {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.filter__preview {
  overflow: hidden;
  max-width: 55%;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.filter__spacer {
  flex: 1;
}

.filter__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
}

.filter__close:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}
</style>
