<script setup lang="ts">
import { ChevronDown, ChevronUp } from 'lucide-vue-next';
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';

import type { DatabaseValue, QueryResultSet } from '@/types/query';

/**
 * 只读结果表格：列名 + 类型、行号、NULL/数字/二进制样式区分。
 * 结果面板与表数据浏览共用；`sortable` 打开列头排序，点击行号可查看整行详情。
 */
const props = withDefaults(
  defineProps<{
    set: QueryResultSet;
    /** 首行的全局序号偏移，分页时让行号连续 */
    rowOffset?: number;
    sortable?: boolean;
    sortColumn?: string | null;
    sortDesc?: boolean;
    /** 允许拖拽列宽（固定布局） */
    resizable?: boolean;
    /** 有未保存变更的行（从 0 开始的行下标） */
    highlightRows?: number[];
    /** 双击单元格进入编辑 */
    editable?: boolean;
    /** 待保存的单元格值，键为 `行:列`（从 0 开始） */
    cellOverrides?: Record<string, string | null>;
    /** 列宽（按列名），用于从偏好设置恢复 */
    columnWidths?: Record<string, number>;
  }>(),
  {
    rowOffset: 0,
    sortable: false,
    sortColumn: null,
    sortDesc: false,
    resizable: false,
    highlightRows: () => [],
    editable: false,
    cellOverrides: () => ({}),
    columnWidths: () => ({}),
  },
);

const DEFAULT_COLUMN_WIDTH = 200;
const MIN_COLUMN_WIDTH = 60;

const widths = ref<Record<number, number>>({});

interface DragState {
  index: number;
  startX: number;
  startWidth: number;
}

let drag: DragState | null = null;

function widthFor(index: number, name: string): number {
  return widths.value[index] ?? props.columnWidths[name] ?? DEFAULT_COLUMN_WIDTH;
}

function handleMove(event: PointerEvent): void {
  if (!drag) {
    return;
  }
  const next = Math.max(
    MIN_COLUMN_WIDTH,
    drag.startWidth + (event.clientX - drag.startX),
  );
  widths.value = { ...widths.value, [drag.index]: next };
}

function stopResize(): void {
  if (drag) {
    const column = props.set.columns[drag.index];
    if (column) {
      emit('column-resize', column.name, widthFor(drag.index, column.name));
    }
  }
  drag = null;
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
  window.removeEventListener('pointermove', handleMove);
  window.removeEventListener('pointerup', stopResize);
}

function startResize(index: number, event: PointerEvent): void {
  const column = props.set.columns[index];
  drag = {
    index,
    startX: event.clientX,
    startWidth: widthFor(index, column?.name ?? ''),
  };
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  window.addEventListener('pointermove', handleMove);
  window.addEventListener('pointerup', stopResize);
}

// 列变化（切换表/结果集）后恢复默认宽度
watch(
  () => props.set.columns.map((column) => column.name).join('|'),
  () => {
    widths.value = {};
  },
);

onBeforeUnmount(stopResize);

const emit = defineEmits<{
  sort: [column: string];
  'row-click': [index: number];
  'cell-commit': [rowIndex: number, columnIndex: number, value: string];
  'column-resize': [name: string, width: number];
}>();

const editingCell = ref<{ row: number; col: number } | null>(null);
const editValue = ref('');
const cellInput = ref<HTMLInputElement | null>(null);

function isEditing(row: number, col: number): boolean {
  return editingCell.value?.row === row && editingCell.value?.col === col;
}

function startCellEdit(row: number, col: number, current: string): void {
  if (!props.editable) {
    return;
  }
  editingCell.value = { row, col };
  editValue.value = current;
  void nextTick(() => {
    cellInput.value?.focus();
    cellInput.value?.select();
  });
}

function commitCell(): void {
  const cell = editingCell.value;
  if (!cell) {
    return;
  }
  const value = editValue.value;
  editingCell.value = null;
  emit('cell-commit', cell.row, cell.col, value);
}

function cancelCell(): void {
  editingCell.value = null;
}

function displayCell(value: DatabaseValue, row: number, col: number): string {
  const key = `${row}:${col}`;
  if (Object.prototype.hasOwnProperty.call(props.cellOverrides, key)) {
    const override = props.cellOverrides[key];
    return override === null || override === undefined ? 'NULL' : override;
  }
  return cellText(value);
}

function isOverridden(row: number, col: number): boolean {
  return Object.prototype.hasOwnProperty.call(props.cellOverrides, `${row}:${col}`);
}

function cellClass(type: string): string {
  if (type === 'null') {
    return 'cell cell--null';
  }
  if (type === 'integer' || type === 'decimal' || type === 'float') {
    return 'cell cell--number';
  }
  if (type === 'binary' || type === 'json') {
    return 'cell cell--muted';
  }
  return 'cell';
}

function cellText(value: DatabaseValue): string {
  if (value.type === 'null') {
    return 'NULL';
  }
  return value.value ?? '';
}
</script>

<template>
  <div class="grid">
    <table class="data-table" :class="{ 'is-fixed': resizable }">
      <colgroup>
        <col style="width: 44px" />
        <col
          v-for="(column, index) in set.columns"
          :key="`col-${index}`"
          :style="resizable ? { width: `${widthFor(index, column.name)}px` } : undefined"
        />
      </colgroup>
      <thead>
        <tr>
          <th class="data-table__index">#</th>
          <th
            v-for="(column, index) in set.columns"
            :key="`${index}-${column.name}`"
            :class="{ 'is-sortable': sortable }"
          >
            <button
              v-if="sortable"
              type="button"
              class="data-table__sort"
              :title="`按 ${column.name} 排序`"
              @click="emit('sort', column.name)"
            >
              <span class="data-table__label">
                <span class="data-table__name">{{ column.name }}</span>
                <span class="data-table__type">{{ column.typeName }}</span>
              </span>
              <ChevronUp
                v-if="sortColumn === column.name && !sortDesc"
                class="data-table__sort-icon"
                :size="11"
              />
              <ChevronDown
                v-else-if="sortColumn === column.name && sortDesc"
                class="data-table__sort-icon"
                :size="11"
              />
            </button>
            <template v-else>
              <span class="data-table__name">{{ column.name }}</span>
              <span class="data-table__type">{{ column.typeName }}</span>
            </template>
            <span
              v-if="resizable"
              class="data-table__resize"
              title="拖拽调整列宽"
              @pointerdown.stop.prevent="startResize(index, $event)"
            ></span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(row, rowIndex) in set.rows"
          :key="rowIndex"
          :class="{ 'is-pending': highlightRows.includes(rowIndex) }"
        >
          <td
            class="data-table__index is-action"
            :title="'查看第 ' + (rowOffset + rowIndex + 1) + ' 行详情'"
            @click="emit('row-click', rowIndex)"
          >
            {{ rowOffset + rowIndex + 1 }}
          </td>
          <td
            v-for="(cell, cellIndex) in row"
            :key="cellIndex"
            :class="[
              cellClass(cell.type),
              {
                'is-editable': editable,
                'is-overridden': isOverridden(rowIndex, cellIndex),
                'is-editing': isEditing(rowIndex, cellIndex),
              },
            ]"
            :title="editable ? '双击编辑' : displayCell(cell, rowIndex, cellIndex)"
            @dblclick="startCellEdit(rowIndex, cellIndex, displayCell(cell, rowIndex, cellIndex))"
          >
            <input
              v-if="isEditing(rowIndex, cellIndex)"
              ref="cellInput"
              v-model="editValue"
              class="cell-editor"
              type="text"
              spellcheck="false"
              autocomplete="off"
              @keydown.enter.prevent="commitCell"
              @keydown.esc.prevent="cancelCell"
              @blur="commitCell"
            />
            <template v-else>{{ displayCell(cell, rowIndex, cellIndex) }}</template>
          </td>
        </tr>
      </tbody>
    </table>

    <p v-if="set.rowCount === 0" class="grid__empty">没有符合条件的记录</p>
  </div>
</template>

<style scoped>
.grid {
  min-height: 0;
  overflow: auto;
}

.data-table {
  width: 100%;
  font-size: var(--font-size-xs);
  user-select: text;
}

.data-table.is-fixed {
  table-layout: fixed;
}

.data-table.is-fixed .cell {
  max-width: none;
}

.data-table__resize {
  position: absolute;
  top: 0;
  right: -3px;
  bottom: 0;
  z-index: 2;
  width: 7px;
  cursor: col-resize;
}

.data-table__resize:hover {
  background-color: var(--primary-color);
  opacity: 0.5;
}

.data-table th,
.data-table td {
  padding: 4px var(--space-2);
  text-align: left;
  white-space: nowrap;
  border-right: 1px solid var(--divider-color);
  border-bottom: 1px solid var(--divider-color);
}

.data-table thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  color: var(--text-secondary);
  font-weight: 500;
  background-color: var(--surface-subtle);
}

.data-table__sort {
  display: flex;
  align-items: center;
  gap: 2px;
  width: 100%;
  text-align: left;
  border-radius: var(--radius-xs);
}

.data-table__sort:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.data-table__sort-icon {
  flex: none;
  color: var(--primary-color);
}

.data-table__label {
  flex: 1;
  min-width: 0;
}

.data-table__name {
  display: block;
}

.data-table__type {
  display: block;
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: 400;
}

.data-table__index {
  width: 40px;
  color: var(--text-tertiary);
  text-align: right;
  background-color: var(--surface-subtle);
}

.data-table__index.is-action {
  cursor: pointer;
}

.data-table__index.is-action:hover {
  color: var(--primary-color);
  text-decoration: underline;
}

.data-table tbody tr.is-pending td {
  background-color: var(--warning-soft);
}

.data-table tbody tr.is-pending .data-table__index {
  color: var(--warning-color);
  font-weight: 600;
}

.data-table tbody tr:hover td {
  background-color: var(--surface-hover);
}

.cell {
  max-width: 420px;
  overflow: hidden;
  color: var(--text-primary);
  text-overflow: ellipsis;
}

.cell--number {
  font-family: var(--font-mono);
  text-align: right;
}

.cell.is-editable {
  cursor: text;
}

.cell.is-overridden {
  color: var(--warning-color);
  background-color: var(--warning-soft);
}

.cell.is-editing {
  padding: 0;
}

.cell-editor {
  width: 100%;
  min-width: 80px;
  height: 22px;
  padding: 0 4px;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  background-color: var(--surface-bg);
  border: 1px solid var(--primary-color);
  border-radius: 2px;
  outline: none;
}

.cell--null {
  color: var(--text-tertiary);
  font-style: italic;
}

.cell--muted {
  color: var(--text-secondary);
}

.grid__empty {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}
</style>
