/**
 * 单元格值的类型校验（按 information_schema 返回的列类型）。
 *
 * 只做「明显错误」的拦截，最终仍由数据库约束兜底；
 * 校验失败时保留原值，避免把非法数据写进待保存队列。
 */

export interface CellValidationInput {
  columnType: string;
  nullable: boolean;
  /** null 表示写入 NULL */
  value: string | null;
}

const INTEGER = /^-?\d+$/;
const DECIMAL = /^-?\d+(\.\d+)?$/;
const FLOAT = /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/;
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const DATETIME = /^\d{4}-\d{2}-\d{2}([ T]\d{2}:\d{2}(:\d{2}(\.\d{1,6})?)?)?$/;
const TIME = /^-?\d{1,3}:\d{2}(:\d{2}(\.\d{1,6})?)?$/;

export function validateCellValue(input: CellValidationInput): string | null {
  const value = input.value;

  if (value === null) {
    return input.nullable ? null : '该列不允许为 NULL';
  }

  const type = input.columnType.trim().toLowerCase();
  const text = value.trim();

  const enumMatch = type.match(/^(?:enum|set)\((.*)\)$/);
  if (enumMatch) {
    const options = enumMatch[1]
      ? enumMatch[1]
          .split(',')
          .map((item) => item.trim().replace(/^'|'$/g, ''))
      : [];
    if (!options.includes(value)) {
      return `只能是：${options.join(' / ')}`;
    }
    return null;
  }

  if (type.startsWith('tinyint(1)') || type === 'boolean' || type === 'bool') {
    if (!['0', '1', 'true', 'false'].includes(text.toLowerCase())) {
      return '布尔列只能是 0/1 或 true/false';
    }
    return null;
  }

  if (type.includes('int')) {
    return INTEGER.test(text) ? null : '需要整数';
  }

  if (type.startsWith('decimal') || type.startsWith('numeric')) {
    return DECIMAL.test(text) ? null : '需要数字（可带小数）';
  }

  if (type.startsWith('float') || type.startsWith('double') || type.startsWith('real')) {
    return FLOAT.test(text) ? null : '需要数字';
  }

  if (type.startsWith('datetime') || type.startsWith('timestamp')) {
    return DATETIME.test(text) ? null : '格式应为 YYYY-MM-DD HH:MM:SS';
  }

  if (type.startsWith('date')) {
    return DATE.test(text) ? null : '格式应为 YYYY-MM-DD';
  }

  if (type.startsWith('time')) {
    return TIME.test(text) ? null : '格式应为 HH:MM:SS';
  }

  if (type.startsWith('year')) {
    return /^\d{4}$/.test(text) ? null : '年份需要 4 位数字';
  }

  if (type.startsWith('json')) {
    try {
      JSON.parse(value);
      return null;
    } catch {
      return 'JSON 格式不正确';
    }
  }

  if (type.startsWith('bit')) {
    return /^[01]+$/.test(text) ? null : 'BIT 列只能是 0 或 1';
  }

  const lengthMatch = type.match(/^(?:var)?char\((\d+)\)$/);
  if (lengthMatch) {
    const max = Number(lengthMatch[1]);
    if ([...value].length > max) {
      return `最多 ${max} 个字符`;
    }
  }

  return null;
}
