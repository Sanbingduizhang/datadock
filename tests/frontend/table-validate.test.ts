import { describe, expect, it } from 'vitest';

import { validateCellValue } from '@/utils/table-validate';

function check(columnType: string, value: string | null, nullable = true): string | null {
  return validateCellValue({ columnType, nullable, value });
}

describe('validateCellValue', () => {
  it('NULL 仅在可空列允许', () => {
    expect(check('varchar(10)', null)).toBeNull();
    expect(check('varchar(10)', null, false)).toContain('NULL');
  });

  it('整数列拒绝小数与空串', () => {
    expect(check('bigint unsigned', '42')).toBeNull();
    expect(check('bigint unsigned', '-42')).toBeNull();
    expect(check('bigint unsigned', '4.2')).toContain('整数');
    expect(check('int', '')).toContain('整数');
  });

  it('decimal 允许小数，拒绝非数字', () => {
    expect(check('decimal(18,4)', '1234.5600')).toBeNull();
    expect(check('decimal(18,4)', 'abc')).toContain('数字');
  });

  it('date / datetime / time 校验格式', () => {
    expect(check('date', '2026-01-31')).toBeNull();
    expect(check('date', '2026/01/31')).toContain('YYYY-MM-DD');
    expect(check('datetime(3)', '2026-01-31 12:30:00.123')).toBeNull();
    expect(check('datetime(3)', '2026-01-31 12:30:00')).toBeNull();
    expect(check('datetime(3)', '31-01-2026')).toContain('YYYY-MM-DD');
    expect(check('time', '12:30:00')).toBeNull();
    expect(check('time', '12:30')).toBeNull();
    expect(check('time', 'noon')).toContain('HH:MM:SS');
  });

  it('JSON 列尝试解析', () => {
    expect(check('json', '{"a":1}')).toBeNull();
    expect(check('json', '{a:1}')).toContain('JSON');
  });

  it('varchar 按字符数限制长度', () => {
    expect(check('varchar(3)', 'abc')).toBeNull();
    expect(check('varchar(3)', 'abcd')).toContain('3');
    expect(check('varchar(2)', '中文')).toBeNull();
  });

  it('enum 只接受列举值', () => {
    expect(check("enum('a','b')", 'a')).toBeNull();
    expect(check("enum('a','b')", 'c')).toContain('a / b');
  });

  it('tinyint(1) 接受布尔写法', () => {
    expect(check('tinyint(1)', '1')).toBeNull();
    expect(check('tinyint(1)', 'true')).toBeNull();
    expect(check('tinyint(1)', '2')).toContain('布尔');
  });

  it('文本列不做格式限制', () => {
    expect(check('text', '任意内容 😀')).toBeNull();
  });
});
