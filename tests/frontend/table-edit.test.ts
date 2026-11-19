import { describe, expect, it } from 'vitest';

import {
  buildDeleteSql,
  buildInsertSql,
  buildUpdateSql,
  buildWhereClause,
  canEditRow,
  toLiteral,
} from '@/utils/table-edit';

import type { FilterCondition, RowEditContext } from '@/utils/table-edit';

const columns = [
  { name: 'id', columnType: 'bigint unsigned' },
  { name: 'username', columnType: 'varchar(64)' },
  { name: 'status', columnType: 'tinyint' },
  { name: 'balance', columnType: 'decimal(18,4)' },
  { name: 'note', columnType: 'text' },
];

function context(overrides: Partial<RowEditContext> = {}): RowEditContext {
  return {
    databaseName: 'tbdock',
    tableName: 'users',
    columns,
    primaryKey: ['id'],
    original: [
      { type: 'integer', value: '1' },
      { type: 'string', value: 'alice' },
      { type: 'integer', value: '1' },
      { type: 'decimal', value: '1234.5600' },
      { type: 'null' },
    ],
    next: ['1', 'alice', '1', '1234.5600', null],
    ...overrides,
  };
}

describe('toLiteral', () => {
  it('数值列不加引号', () => {
    expect(toLiteral('1', 'bigint unsigned')).toBe('1');
    expect(toLiteral('1234.5600', 'decimal(18,4)')).toBe('1234.5600');
  });

  it('非数值列即使内容是数字也加引号', () => {
    expect(toLiteral('1', 'varchar(10)')).toBe("'1'");
  });

  it('NULL 输出为 NULL 关键字', () => {
    expect(toLiteral(null, 'text')).toBe('NULL');
  });

  it('转义单引号与反斜杠', () => {
    expect(toLiteral("it's", 'varchar(20)')).toBe("'it\\'s'");
    expect(toLiteral('a\\b', 'varchar(20)')).toBe("'a\\\\b'");
    expect(toLiteral('', 'varchar(20)')).toBe("''");
  });
});

describe('buildUpdateSql', () => {
  it('只更新发生变化的列，并按主键限一行', () => {
    const sql = buildUpdateSql(context({ next: ['1', 'alice2', '1', '1234.5600', 'hi'] }));

    expect(sql).toBe(
      "UPDATE `tbdock`.`users` SET `username` = 'alice2', `note` = 'hi' WHERE `id` = 1 LIMIT 1;",
    );
  });

  it('没有变化时返回 null', () => {
    expect(buildUpdateSql(context())).toBeNull();
  });

  it('把字段改为 NULL', () => {
    const sql = buildUpdateSql(context({ next: ['1', null, '1', '1234.5600', null] }));

    expect(sql).toContain('`username` = NULL');
  });

  it('没有主键时拒绝生成', () => {
    expect(buildUpdateSql(context({ primaryKey: [] }))).toBeNull();
  });

  it('复合主键全部进入 WHERE', () => {
    const composite = context({
      columns: [
        { name: 'a', columnType: 'int' },
        { name: 'b', columnType: 'varchar(8)' },
      ],
      primaryKey: ['a', 'b'],
      original: [
        { type: 'integer', value: '1' },
        { type: 'string', value: 'x' },
      ],
      next: ['1', 'y'],
    });

    expect(buildUpdateSql(composite)).toBe(
      "UPDATE `tbdock`.`users` SET `b` = 'y' WHERE `a` = 1 AND `b` = 'x' LIMIT 1;",
    );
  });
});

describe('buildDeleteSql', () => {
  it('生成带主键与 LIMIT 1 的删除语句', () => {
    expect(buildDeleteSql(context())).toBe(
      'DELETE FROM `tbdock`.`users` WHERE `id` = 1 LIMIT 1;',
    );
  });

  it('主键值缺失时拒绝生成', () => {
    expect(
      buildDeleteSql(
        context({
          original: [{ type: 'null' }, ...context().original.slice(1)],
        }),
      ),
    ).toBeNull();
  });
});

describe('canEditRow', () => {
  it('有完整主键时可编辑', () => {
    expect(canEditRow(context())).toBe(true);
  });

  it('没有主键或主键为空时不可编辑', () => {
    expect(canEditRow(context({ primaryKey: [] }))).toBe(false);
    expect(
      canEditRow(
        context({ original: [{ type: 'null' }, ...context().original.slice(1)] }),
      ),
    ).toBe(false);
  });
});

describe('buildInsertSql', () => {
  it('跳过默认值列，只为赋值列生成列表', () => {
    const sql = buildInsertSql('tbdock', 'users', [
      { column: columns[0]!, mode: 'default', value: '' },
      { column: columns[1]!, mode: 'value', value: 'bob' },
      { column: columns[2]!, mode: 'value', value: '1' },
      { column: columns[3]!, mode: 'null', value: '' },
    ]);

    expect(sql).toBe(
      "INSERT INTO `tbdock`.`users` (`username`, `status`, `balance`) VALUES ('bob', 1, NULL);",
    );
  });

  it('全部使用默认值时返回 null', () => {
    expect(
      buildInsertSql('tbdock', 'users', [
        { column: columns[0]!, mode: 'default', value: '' },
      ]),
    ).toBeNull();
  });

  it('字符串列按字符串字面量写入，含转义', () => {
    const sql = buildInsertSql('tbdock', 'users', [
      { column: columns[1]!, mode: 'value', value: "o'brien" },
    ]);

    expect(sql).toBe("INSERT INTO `tbdock`.`users` (`username`) VALUES ('o\\'brien');");
  });
});

function condition(overrides: Partial<FilterCondition>): FilterCondition {
  return {
    column: 'username',
    columnType: 'varchar(64)',
    operator: 'eq',
    value: 'alice',
    connector: 'AND',
    ...overrides,
  };
}

describe('buildWhereClause', () => {
  it('单条件等值比较', () => {
    expect(buildWhereClause([condition({})])).toBe("`username` = 'alice'");
  });

  it('数值列不加引号', () => {
    expect(
      buildWhereClause([condition({ column: 'status', columnType: 'tinyint', value: '1' })]),
    ).toBe('`status` = 1');
  });

  it('多个条件按连接符拼接', () => {
    const where = buildWhereClause([
      condition({ column: 'status', columnType: 'tinyint', value: '1' }),
      condition({ operator: 'contains', value: 'ali', connector: 'OR' }),
    ]);

    expect(where).toBe("`status` = 1 OR `username` LIKE '%ali%'");
  });

  it('NULL 条件不需要值', () => {
    expect(buildWhereClause([condition({ operator: 'isNull' })])).toBe('`username` IS NULL');
    expect(buildWhereClause([condition({ operator: 'isNotNull', value: '' })])).toBe(
      '`username` IS NOT NULL',
    );
  });

  it('IN 列表拆分为多个字面量', () => {
    expect(
      buildWhereClause([
        condition({ column: 'id', columnType: 'bigint', operator: 'in', value: '1, 2,3' }),
      ]),
    ).toBe('`id` IN (1, 2, 3)');
  });

  it('LIKE 中的百分号被转义，避免用户输入变成通配符', () => {
    expect(buildWhereClause([condition({ operator: 'contains', value: '100%' })])).toBe(
      "`username` LIKE '%100\\%%'",
    );
  });

  it('空值条件被忽略，全为空时返回 null', () => {
    expect(buildWhereClause([condition({ value: '  ' })])).toBeNull();
    expect(buildWhereClause([])).toBeNull();
  });
});
