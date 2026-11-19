/**
 * 表数据行编辑的 SQL 生成。
 *
 * 只根据主键定位单行，并强制 `LIMIT 1`；生成结果会先展示给用户确认再执行。
 * 这些函数是纯函数，便于单测覆盖转义与类型判断。
 */

import type { DatabaseValue } from '@/types/query';

export interface EditColumn {
  name: string;
  columnType: string;
}

export interface RowEditContext {
  databaseName: string;
  tableName: string;
  columns: EditColumn[];
  primaryKey: string[];
  /** 原始行（与 columns 顺序一致） */
  original: DatabaseValue[];
  /** 编辑后的值，null 表示 NULL */
  next: (string | null)[];
}

/** 数值类型且内容可解析为数字时不加引号，其余一律按字符串字面量 */
const NUMERIC_TYPE = /^(tinyint|smallint|mediumint|int|integer|bigint|decimal|numeric|float|double|real|year)/i;
const NUMERIC_VALUE = /^-?\d+(\.\d+)?$/;

export function quoteIdentifier(name: string): string {
  return `\`${name.replace(/`/g, '``')}\``;
}

export function toLiteral(value: string | null, columnType: string): string {
  if (value === null) {
    return 'NULL';
  }
  if (NUMERIC_TYPE.test(columnType.trim()) && NUMERIC_VALUE.test(value)) {
    return value;
  }
  const escaped = value.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
  return `'${escaped}'`;
}

function qualifiedTable(context: RowEditContext): string {
  return `${quoteIdentifier(context.databaseName)}.${quoteIdentifier(context.tableName)}`;
}

function buildWhere(context: RowEditContext): string | null {
  const conditions: string[] = [];

  for (const key of context.primaryKey) {
    const index = context.columns.findIndex((column) => column.name === key);
    if (index < 0) {
      return null;
    }
    const cell = context.original[index];
    if (!cell || cell.type === 'null') {
      return null;
    }
    conditions.push(`${quoteIdentifier(key)} = ${toLiteral(cell.value ?? '', context.columns[index]?.columnType ?? '')}`);
  }

  if (conditions.length === 0) {
    return null;
  }

  return `${conditions.join(' AND ')} LIMIT 1`;
}

function sameValue(original: DatabaseValue | undefined, next: string | null): boolean {
  if (!original || original.type === 'null') {
    return next === null;
  }
  return next !== null && next === original.value;
}

/** 生成 UPDATE；没有变化或缺少主键时返回 null */
export function buildUpdateSql(context: RowEditContext): string | null {
  const where = buildWhere(context);
  if (!where) {
    return null;
  }

  const assignments: string[] = [];
  context.columns.forEach((column, index) => {
    const next = context.next[index] ?? null;
    if (sameValue(context.original[index], next)) {
      return;
    }
    assignments.push(`${quoteIdentifier(column.name)} = ${toLiteral(next, column.columnType)}`);
  });

  if (assignments.length === 0) {
    return null;
  }

  return `UPDATE ${qualifiedTable(context)} SET ${assignments.join(', ')} WHERE ${where};`;
}

/** 生成 DELETE；缺少主键时返回 null */
export function buildDeleteSql(context: RowEditContext): string | null {
  const where = buildWhere(context);
  if (!where) {
    return null;
  }

  return `DELETE FROM ${qualifiedTable(context)} WHERE ${where};`;
}

/** 判断当前是否可编辑：必须有主键且主键值完整 */
export function canEditRow(context: Pick<RowEditContext, 'primaryKey' | 'columns' | 'original'>): boolean {
  if (context.primaryKey.length === 0) {
    return false;
  }

  return context.primaryKey.every((key) => {
    const index = context.columns.findIndex((column) => column.name === key);
    const cell = context.original[index];
    return index >= 0 && cell !== undefined && cell.type !== 'null';
  });
}

/* ---------- 新增行 ---------- */

export type InsertValueMode = 'default' | 'null' | 'value';

export interface InsertField {
  column: EditColumn;
  mode: InsertValueMode;
  value: string;
}

/**
 * 生成 INSERT 语句。
 * - `default` 的列不出现在列表里，交给数据库默认值（含自增主键）；
 * - `null` 的列显式写 NULL；
 * - 全部为 `default` 时返回 null（没有可插入的列）。
 */
export function buildInsertSql(
  databaseName: string,
  tableName: string,
  fields: InsertField[],
): string | null {
  const used = fields.filter((field) => field.mode !== 'default');
  if (used.length === 0) {
    return null;
  }

  const columns = used.map((field) => quoteIdentifier(field.column.name)).join(', ');
  const values = used
    .map((field) =>
      field.mode === 'null' ? 'NULL' : toLiteral(field.value, field.column.columnType),
    )
    .join(', ');

  return `INSERT INTO ${quoteIdentifier(databaseName)}.${quoteIdentifier(tableName)} (${columns}) VALUES (${values});`;
}

/* ---------- 条件筛选 ---------- */

export type FilterOperator =
  | 'eq'
  | 'ne'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'contains'
  | 'notContains'
  | 'startsWith'
  | 'endsWith'
  | 'in'
  | 'isNull'
  | 'isNotNull';

export interface FilterCondition {
  column: string;
  columnType: string;
  operator: FilterOperator;
  value: string;
  /** 与「上一行」的连接符 */
  connector: 'AND' | 'OR';
}

export const FILTER_OPERATORS: { value: FilterOperator; label: string; needValue: boolean }[] = [
  { value: 'eq', label: '=', needValue: true },
  { value: 'ne', label: '≠', needValue: true },
  { value: 'gt', label: '>', needValue: true },
  { value: 'gte', label: '≥', needValue: true },
  { value: 'lt', label: '<', needValue: true },
  { value: 'lte', label: '≤', needValue: true },
  { value: 'contains', label: '包含', needValue: true },
  { value: 'notContains', label: '不包含', needValue: true },
  { value: 'startsWith', label: '开头是', needValue: true },
  { value: 'endsWith', label: '结尾是', needValue: true },
  { value: 'in', label: '在列表中（逗号分隔）', needValue: true },
  { value: 'isNull', label: '为 NULL', needValue: false },
  { value: 'isNotNull', label: '不为 NULL', needValue: false },
];

function likeLiteral(value: string, pattern: 'contains' | 'startsWith' | 'endsWith'): string {
  const escaped = value.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/%/g, '\\%');
  switch (pattern) {
    case 'contains':
      return `'%${escaped}%'`;
    case 'startsWith':
      return `'${escaped}%'`;
    default:
      return `'%${escaped}'`;
  }
}

/** 把单条条件转成 SQL 片段；缺少值或列名时返回 null */
export function buildCondition(condition: FilterCondition): string | null {
  const column = quoteIdentifier(condition.column);

  switch (condition.operator) {
    case 'isNull':
      return `${column} IS NULL`;
    case 'isNotNull':
      return `${column} IS NOT NULL`;
    case 'contains':
      return `${column} LIKE ${likeLiteral(condition.value, 'contains')}`;
    case 'notContains':
      return `${column} NOT LIKE ${likeLiteral(condition.value, 'contains')}`;
    case 'startsWith':
      return `${column} LIKE ${likeLiteral(condition.value, 'startsWith')}`;
    case 'endsWith':
      return `${column} LIKE ${likeLiteral(condition.value, 'endsWith')}`;
    case 'in': {
      const items = condition.value
        .split(',')
        .map((item) => item.trim())
        .filter((item) => item.length > 0)
        .map((item) => toLiteral(item, condition.columnType));
      return items.length === 0 ? null : `${column} IN (${items.join(', ')})`;
    }
    default:
      break;
  }

  if (condition.value.trim().length === 0) {
    return null;
  }

  const operators: Record<string, string> = {
    eq: '=',
    ne: '<>',
    gt: '>',
    gte: '>=',
    lt: '<',
    lte: '<=',
  };

  return `${column} ${operators[condition.operator]} ${toLiteral(condition.value, condition.columnType)}`;
}

/** 组合多个条件；没有有效条件时返回 null */
export function buildWhereClause(conditions: FilterCondition[]): string | null {
  const parts: string[] = [];

  conditions.forEach((condition, index) => {
    const sql = buildCondition(condition);
    if (!sql) {
      return;
    }
    parts.push(index === 0 ? sql : `${condition.connector} ${sql}`);
  });

  return parts.length === 0 ? null : parts.join(' ');
}
