/**
 * 表结构设计器的草稿模型与 ALTER 语句生成。
 *
 * 设计原则：
 * - 只生成「差异」，未改动的字段/索引不会出现在语句里，避免无意义的 MODIFY；
 * - 每个变更单独一条 `ALTER TABLE`，出错时能定位到具体字段；
 * - 不做类型推断，类型串按用户输入原样使用（`varchar(64)`、`decimal(12,2)` 等）；
 * - 语句顺序：删索引 → 删主键 → 改字段 → 加主键 → 加索引，避免索引引用已删除的列。
 */

import type { TableSchema } from '@/types/database';

export interface ColumnDraft {
  /** 稳定标识：编辑过程中不变，用于识别同一行 */
  key: string;
  /** 原字段名；null 表示这是一个新增字段 */
  originalName: string | null;
  name: string;
  /** 完整类型，例如 varchar(64) */
  columnType: string;
  nullable: boolean;
  /** null 表示不指定 DEFAULT（交给数据库默认行为） */
  defaultValue: string | null;
  comment: string;
  primary: boolean;
  autoIncrement: boolean;
  /** `ON UPDATE CURRENT_TIMESTAMP` 的表达式，需原样保留，否则 MODIFY 会丢失 */
  onUpdate: string | null;
}

export interface IndexDraft {
  key: string;
  /** 原索引名；null 表示新增 */
  originalName: string | null;
  name: string;
  unique: boolean;
  /** 逗号分隔的列名 */
  columns: string;
}

export interface SchemaDraftInput {
  database: string;
  table: string;
  originalColumns: ColumnDraft[];
  originalIndexes: IndexDraft[];
  originalPrimaryKey: string[];
  columns: ColumnDraft[];
  indexes: IndexDraft[];
}

/** 常用 MySQL 字段类型，按类别分组供下拉选择 */
export const COLUMN_TYPE_GROUPS: { label: string; types: string[] }[] = [
  {
    label: '整数',
    types: ['TINYINT', 'SMALLINT', 'MEDIUMINT', 'INT', 'BIGINT'],
  },
  {
    label: '小数',
    types: ['DECIMAL', 'FLOAT', 'DOUBLE'],
  },
  {
    label: '字符串',
    types: ['VARCHAR', 'CHAR', 'TINYTEXT', 'TEXT', 'MEDIUMTEXT', 'LONGTEXT'],
  },
  {
    label: '日期时间',
    types: ['DATE', 'DATETIME', 'TIMESTAMP', 'TIME', 'YEAR'],
  },
  {
    label: '其他',
    types: ['BOOLEAN', 'JSON', 'BLOB', 'MEDIUMBLOB', 'LONGBLOB', 'BINARY', 'VARBINARY', 'BIT'],
  },
];

/** 需要长度/精度参数的类型（其它的长度输入框置灰） */
const LENGTH_TYPES = new Set([
  'varchar',
  'char',
  'binary',
  'varbinary',
  'tinyint',
  'smallint',
  'mediumint',
  'int',
  'integer',
  'bigint',
  'decimal',
  'numeric',
  'float',
  'double',
  'bit',
]);

export interface ParsedColumnType {
  /** 基础类型，如 varchar */
  base: string;
  /** 括号内的参数，如 "255" 或 "12,2"；无参为 null */
  args: string | null;
}

/** 把完整类型串解析为 base + args，例如 `decimal(12,2)` → `{ base: 'decimal', args: '12,2' }` */
export function parseColumnType(columnType: string): ParsedColumnType {
  const trimmed = columnType.trim();
  const matched = trimmed.match(/^([a-zA-Z]+)\s*(?:\(([^)]*)\))?/);
  if (!matched) {
    return { base: trimmed.toLowerCase(), args: null };
  }
  return {
    base: matched[1].toLowerCase(),
    args: matched[2]?.trim() || null,
  };
}

/** 组合 base + 长度参数为完整类型串；length 为空时只返回 base */
export function composeColumnType(base: string, length: string | null): string {
  const cleanBase = base.trim();
  if (!cleanBase) {
    return '';
  }
  const trimmedLength = length?.trim() ?? '';
  return trimmedLength.length > 0 ? `${cleanBase}(${trimmedLength})` : cleanBase;
}

/** 该基础类型是否需要长度/精度输入 */
export function typeNeedsLength(base: string): boolean {
  return LENGTH_TYPES.has(base.trim().toLowerCase());
}

/** 反引号包裹标识符，内部反引号翻倍 */
export function quoteIdentifier(name: string): string {
  return `\`${name.replace(/`/g, '``')}\``;
}

function escapeString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
}

function isNumericType(columnType: string): boolean {
  const base = columnType.trim().toLowerCase().split(/[\s(]/)[0] ?? '';
  return [
    'bit',
    'tinyint',
    'smallint',
    'mediumint',
    'int',
    'integer',
    'bigint',
    'decimal',
    'numeric',
    'float',
    'double',
    'real',
    'year',
  ].includes(base);
}

/** 把用户输入的默认值转成 `DEFAULT ...` 片段；返回 null 表示不写 */
export function formatDefault(value: string | null, columnType: string): string | null {
  if (value === null) {
    return null;
  }

  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return null;
  }

  const upper = trimmed.toUpperCase();
  if (upper === 'NULL') {
    return 'DEFAULT NULL';
  }
  if (upper.startsWith('CURRENT_TIMESTAMP') || upper === 'NOW()') {
    return `DEFAULT ${trimmed}`;
  }
  // 表达式默认值（MySQL 8）：DEFAULT (uuid())
  if (trimmed.startsWith('(') && trimmed.endsWith(')')) {
    return `DEFAULT ${trimmed}`;
  }
  if (isNumericType(columnType) && /^-?\d+(\.\d+)?$/.test(trimmed)) {
    return `DEFAULT ${trimmed}`;
  }

  return `DEFAULT '${escapeString(trimmed)}'`;
}

/** 生成单列定义，用于 MODIFY / CHANGE / ADD / CREATE */
export function buildColumnDefinition(draft: ColumnDraft): string {
  const parts: string[] = [
    quoteIdentifier(draft.name.trim()),
    draft.columnType.trim() || 'varchar(255)',
    draft.nullable ? 'NULL' : 'NOT NULL',
  ];

  const defaultPart = formatDefault(draft.defaultValue, draft.columnType);
  if (defaultPart) {
    parts.push(defaultPart);
  }
  if (draft.autoIncrement) {
    parts.push('AUTO_INCREMENT');
  }
  if (draft.onUpdate) {
    parts.push(`ON UPDATE ${draft.onUpdate}`);
  }
  if (draft.comment.trim().length > 0) {
    parts.push(`COMMENT '${escapeString(draft.comment.trim())}'`);
  }

  return parts.join(' ');
}

/** 从表结构读出草稿 */
export function toColumnDrafts(schema: TableSchema): ColumnDraft[] {
  return schema.columns.map((column, index) => ({
    key: `col-${index}-${column.name}`,
    originalName: column.name,
    name: column.name,
    columnType: column.columnType,
    nullable: column.nullable,
    defaultValue: column.defaultValue ?? null,
    comment: column.comment,
    primary: column.columnKey.toUpperCase() === 'PRI',
    autoIncrement: column.extra.toLowerCase().includes('auto_increment'),
    onUpdate: extractOnUpdate(column.extra),
  }));
}

/** 从信息架构的 EXTRA 中提取 `on update CURRENT_TIMESTAMP[(n)]` */
function extractOnUpdate(extra: string): string | null {
  const matched = extra.match(/on update\s+(current_timestamp(?:\(\d+\))?)/i);
  return matched?.[1] ?? null;
}

export function toIndexDrafts(schema: TableSchema): IndexDraft[] {
  return schema.indexes
    .filter((index) => index.name.toUpperCase() !== 'PRIMARY')
    .map((index, position) => ({
      key: `idx-${position}-${index.name}`,
      originalName: index.name,
      name: index.name,
      unique: index.unique,
      columns: index.columns.join(', '),
    }));
}

function splitColumns(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function sameStringSet(left: string[], right: string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }
  const sortedLeft = [...left].sort();
  const sortedRight = [...right].sort();
  return sortedLeft.every((item, index) => item === sortedRight[index]);
}

function hasColumnChanged(original: ColumnDraft, draft: ColumnDraft): boolean {
  return (
    original.name !== draft.name.trim() ||
    original.columnType.trim().toLowerCase() !== draft.columnType.trim().toLowerCase() ||
    original.nullable !== draft.nullable ||
    (original.defaultValue ?? '') !== (draft.defaultValue ?? '') ||
    original.comment.trim() !== draft.comment.trim() ||
    original.autoIncrement !== draft.autoIncrement ||
    (original.onUpdate ?? '') !== (draft.onUpdate ?? '')
  );
}

function hasIndexChanged(original: IndexDraft, draft: IndexDraft): boolean {
  return (
    original.name !== draft.name.trim() ||
    original.unique !== draft.unique ||
    !sameStringSet(splitColumns(original.columns), splitColumns(draft.columns))
  );
}

/** 前端可执行的基础校验，返回错误消息；通过则返回 null */
export function validateDrafts(columns: ColumnDraft[], indexes: IndexDraft[]): string | null {
  const names = new Set<string>();

  for (const column of columns) {
    const name = column.name.trim();
    if (name.length === 0) {
      return '字段名不能为空';
    }
    if (column.columnType.trim().length === 0) {
      return `字段 ${name} 缺少类型`;
    }
    if (names.has(name.toLowerCase())) {
      return `字段名重复：${name}`;
    }
    names.add(name.toLowerCase());
  }

  const indexNames = new Set<string>();
  for (const index of indexes) {
    const name = index.name.trim();
    if (name.length === 0) {
      return '索引名不能为空';
    }
    if (indexNames.has(name.toLowerCase())) {
      return `索引名重复：${name}`;
    }
    indexNames.add(name.toLowerCase());

    const referenced = splitColumns(index.columns);
    if (referenced.length === 0) {
      return `索引 ${name} 未指定字段`;
    }
    for (const column of referenced) {
      if (!names.has(column.toLowerCase())) {
        return `索引 ${name} 引用了不存在的字段：${column}`;
      }
    }
  }

  return null;
}

/**
 * 对比原结构与草稿，生成 ALTER TABLE 语句列表。
 * 没有变化时返回空数组。
 */
export function buildSchemaChanges(input: SchemaDraftInput): string[] {
  const statements: string[] = [];
  const table = `${quoteIdentifier(input.database)}.${quoteIdentifier(input.table)}`;

  const draftIndexNames = new Set(
    input.indexes.filter((index) => index.originalName).map((index) => index.originalName),
  );

  // 1. 删除索引（先删，避免字段删除后索引引用失效）
  for (const original of input.originalIndexes) {
    if (!draftIndexNames.has(original.name)) {
      statements.push(`ALTER TABLE ${table} DROP INDEX ${quoteIdentifier(original.name)};`);
    }
  }

  // 2. 修改索引：DROP + ADD（MySQL 不支持原地改索引）
  const rebuilds: IndexDraft[] = [];
  for (const draft of input.indexes) {
    if (!draft.originalName) {
      continue;
    }
    const original = input.originalIndexes.find((item) => item.name === draft.originalName);
    if (original && hasIndexChanged(original, draft)) {
      statements.push(
        `ALTER TABLE ${table} DROP INDEX ${quoteIdentifier(draft.originalName)};`,
      );
      rebuilds.push(draft);
    }
  }

  // 3. 主键变化：先删（新增主键放到字段变更之后）
  const newPrimaryKey = input.columns
    .filter((column) => column.primary)
    .map((column) => column.name.trim());
  const primaryChanged = !sameStringSet(input.originalPrimaryKey, newPrimaryKey);
  if (primaryChanged && input.originalPrimaryKey.length > 0) {
    statements.push(`ALTER TABLE ${table} DROP PRIMARY KEY;`);
  }

  // 4. 删除字段
  for (const original of input.originalColumns) {
    if (!input.columns.some((column) => column.originalName === original.name)) {
      statements.push(`ALTER TABLE ${table} DROP COLUMN ${quoteIdentifier(original.name)};`);
    }
  }

  // 5. 修改已有字段
  for (const draft of input.columns) {
    if (!draft.originalName) {
      continue;
    }
    const original = input.originalColumns.find((item) => item.name === draft.originalName);
    if (!original || !hasColumnChanged(original, draft)) {
      continue;
    }

    const definition = buildColumnDefinition({ ...draft, primary: false });
    if (draft.name.trim() !== original.name) {
      statements.push(
        `ALTER TABLE ${table} CHANGE COLUMN ${quoteIdentifier(original.name)} ${definition};`,
      );
    } else {
      statements.push(`ALTER TABLE ${table} MODIFY COLUMN ${definition};`);
    }
  }

  // 6. 新增字段：按草稿顺序定位，保证列顺序符合用户预期
  input.columns.forEach((draft, index) => {
    if (draft.originalName) {
      return;
    }
    const definition = buildColumnDefinition({ ...draft, primary: false });
    const previous = index > 0 ? input.columns[index - 1]?.name.trim() : null;
    const position = previous ? ` AFTER ${quoteIdentifier(previous)}` : ' FIRST';
    statements.push(`ALTER TABLE ${table} ADD COLUMN ${definition}${position};`);
  });

  // 7. 新增主键
  if (primaryChanged && newPrimaryKey.length > 0) {
    statements.push(
      `ALTER TABLE ${table} ADD PRIMARY KEY (${newPrimaryKey.map(quoteIdentifier).join(', ')});`,
    );
  }

  // 8. 重建被修改的索引
  for (const draft of rebuilds) {
    const columns = splitColumns(draft.columns).map(quoteIdentifier).join(', ');
    if (columns.length === 0) {
      continue;
    }
    statements.push(
      `ALTER TABLE ${table} ADD ${draft.unique ? 'UNIQUE ' : ''}INDEX ${quoteIdentifier(
        draft.name.trim(),
      )} (${columns});`,
    );
  }

  // 9. 新增索引
  for (const draft of input.indexes) {
    if (draft.originalName) {
      continue;
    }
    const columns = splitColumns(draft.columns).map(quoteIdentifier).join(', ');
    if (columns.length === 0) {
      continue;
    }
    statements.push(
      `ALTER TABLE ${table} ADD ${draft.unique ? 'UNIQUE ' : ''}INDEX ${quoteIdentifier(
        draft.name.trim(),
      )} (${columns});`,
    );
  }

  return statements;
}
