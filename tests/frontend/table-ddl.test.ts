import { describe, expect, it } from 'vitest';

import {
  buildColumnDefinition,
  buildSchemaChanges,
  formatDefault,
  toColumnDrafts,
  toIndexDrafts,
  validateDrafts,
} from '@/utils/table-ddl';

import type { ColumnDraft, IndexDraft, SchemaDraftInput } from '@/utils/table-ddl';
import type { TableSchema } from '@/types/database';

function column(overrides: Partial<ColumnDraft> = {}): ColumnDraft {
  return {
    key: 'col-1',
    originalName: 'username',
    name: 'username',
    columnType: 'varchar(64)',
    nullable: false,
    defaultValue: null,
    comment: '',
    primary: false,
    autoIncrement: false,
    onUpdate: null,
    ...overrides,
  };
}

function index(overrides: Partial<IndexDraft> = {}): IndexDraft {
  return {
    key: 'idx-1',
    originalName: 'idx_username',
    name: 'idx_username',
    unique: false,
    columns: 'username',
    ...overrides,
  };
}

function input(overrides: Partial<SchemaDraftInput> = {}): SchemaDraftInput {
  return {
    database: 'tbdock',
    table: 'users',
    originalColumns: [column()],
    originalIndexes: [index()],
    originalPrimaryKey: [],
    columns: [column()],
    indexes: [index()],
    ...overrides,
  };
}

describe('formatDefault', () => {
  it('空值与 null 不写 DEFAULT', () => {
    expect(formatDefault(null, 'varchar(64)')).toBeNull();
    expect(formatDefault('   ', 'varchar(64)')).toBeNull();
  });

  it('数值列不加引号', () => {
    expect(formatDefault('0', 'int')).toBe('DEFAULT 0');
    expect(formatDefault('-1.5', 'decimal(10,2)')).toBe('DEFAULT -1.5');
  });

  it('字符串列加引号并转义', () => {
    expect(formatDefault("o'brien", 'varchar(64)')).toBe("DEFAULT 'o\\'brien'");
  });

  it('CURRENT_TIMESTAMP 与表达式保持原样', () => {
    expect(formatDefault('CURRENT_TIMESTAMP', 'datetime')).toBe('DEFAULT CURRENT_TIMESTAMP');
    expect(formatDefault('(uuid())', 'char(36)')).toBe('DEFAULT (uuid())');
  });

  it('显式 NULL 保留', () => {
    expect(formatDefault('NULL', 'varchar(64)')).toBe('DEFAULT NULL');
  });
});

describe('buildColumnDefinition', () => {
  it('拼接类型、可空、默认值、注释', () => {
    expect(
      buildColumnDefinition(
        column({ nullable: true, defaultValue: 'hello', comment: '用户名' }),
      ),
    ).toBe("`username` varchar(64) NULL DEFAULT 'hello' COMMENT '用户名'");
  });

  it('保留自增属性', () => {
    expect(
      buildColumnDefinition(
        column({ name: 'id', columnType: 'bigint unsigned', autoIncrement: true }),
      ),
    ).toBe('`id` bigint unsigned NOT NULL AUTO_INCREMENT');
  });

  it('缺少类型时回退到 varchar(255)，避免生成非法语句', () => {
    expect(buildColumnDefinition(column({ columnType: '  ' }))).toContain('varchar(255)');
  });
});

describe('buildSchemaChanges', () => {
  it('没有变化时返回空数组', () => {
    expect(buildSchemaChanges(input())).toEqual([]);
  });

  it('改字段名使用 CHANGE COLUMN，保留旧名定位', () => {
    const statements = buildSchemaChanges(
      input({ columns: [column({ name: 'user_name' })] }),
    );
    expect(statements).toEqual([
      'ALTER TABLE `tbdock`.`users` CHANGE COLUMN `username` `user_name` varchar(64) NOT NULL;',
    ]);
  });

  it('改类型使用 MODIFY COLUMN', () => {
    const statements = buildSchemaChanges(
      input({ columns: [column({ columnType: 'varchar(128)', nullable: true })] }),
    );
    expect(statements).toEqual([
      'ALTER TABLE `tbdock`.`users` MODIFY COLUMN `username` varchar(128) NULL;',
    ]);
  });

  it('新增字段按草稿顺序使用 AFTER / FIRST', () => {
    const statements = buildSchemaChanges(
      input({
        columns: [
          column({ key: 'new-1', originalName: null, name: 'email', columnType: 'varchar(120)' }),
          column(),
        ],
      }),
    );
    expect(statements).toEqual([
      'ALTER TABLE `tbdock`.`users` ADD COLUMN `email` varchar(120) NOT NULL FIRST;',
    ]);
  });

  it('删除字段生成 DROP COLUMN', () => {
    const statements = buildSchemaChanges(input({ columns: [] }));
    expect(statements).toContain('ALTER TABLE `tbdock`.`users` DROP COLUMN `username`;');
  });

  it('主键变化时先删后加', () => {
    const statements = buildSchemaChanges(
      input({
        originalColumns: [column({ name: 'id', columnType: 'bigint', primary: true })],
        originalPrimaryKey: ['id'],
        columns: [
          column({ key: 'id', originalName: 'id', name: 'id', columnType: 'bigint', primary: true }),
          column({ key: 'u', primary: true }),
        ],
      }),
    );
    expect(statements).toContain('ALTER TABLE `tbdock`.`users` DROP PRIMARY KEY;');
    expect(statements).toContain(
      'ALTER TABLE `tbdock`.`users` ADD PRIMARY KEY (`id`, `username`);',
    );
  });

  it('删除索引生成 DROP INDEX，且排在字段变更之前', () => {
    const statements = buildSchemaChanges(
      input({ indexes: [], columns: [column({ name: 'user_name' })] }),
    );
    expect(statements[0]).toBe('ALTER TABLE `tbdock`.`users` DROP INDEX `idx_username`;');
  });

  it('新增唯一索引', () => {
    const statements = buildSchemaChanges(
      input({
        indexes: [
          index(),
          index({ key: 'new-idx', originalName: null, name: 'uk_email', unique: true, columns: 'username' }),
        ],
      }),
    );
    expect(statements).toEqual([
      'ALTER TABLE `tbdock`.`users` ADD UNIQUE INDEX `uk_email` (`username`);',
    ]);
  });

  it('修改索引等于先删后加', () => {
    const statements = buildSchemaChanges(
      input({ indexes: [index({ unique: true })] }),
    );
    expect(statements).toEqual([
      'ALTER TABLE `tbdock`.`users` DROP INDEX `idx_username`;',
      'ALTER TABLE `tbdock`.`users` ADD UNIQUE INDEX `idx_username` (`username`);',
    ]);
  });
});

describe('validateDrafts', () => {
  it('字段名为空时报错', () => {
    expect(validateDrafts([column({ name: '  ' })], [])).toBe('字段名不能为空');
  });

  it('字段重名时报错（忽略大小写）', () => {
    expect(validateDrafts([column(), column({ key: 'b', name: 'USERNAME' })], [])).toBe(
      '字段名重复：USERNAME',
    );
  });

  it('索引引用不存在字段时报错', () => {
    expect(validateDrafts([column()], [index({ columns: 'email' })])).toBe(
      '索引 idx_username 引用了不存在的字段：email',
    );
  });

  it('校验通过时返回 null', () => {
    expect(validateDrafts([column()], [index()])).toBeNull();
  });
});

describe('草稿转换', () => {
  const schema: TableSchema = {
    columns: [
      {
        name: 'id',
        columnType: 'bigint unsigned',
        nullable: false,
        defaultValue: null,
        columnKey: 'PRI',
        extra: 'auto_increment',
        comment: '',
        position: 1,
      },
      {
        name: 'username',
        columnType: 'varchar(64)',
        nullable: true,
        defaultValue: 'guest',
        columnKey: 'MUL',
        extra: '',
        comment: '登录名',
        position: 2,
      },
    ],
    indexes: [
      { name: 'PRIMARY', unique: true, indexType: 'BTREE', columns: ['id'] },
      { name: 'idx_username', unique: false, indexType: 'BTREE', columns: ['username'] },
    ],
    ddl: 'CREATE TABLE `users` (...)',
  };

  it('识别主键与自增', () => {
    const drafts = toColumnDrafts(schema);
    expect(drafts[0]?.primary).toBe(true);
    expect(drafts[0]?.autoIncrement).toBe(true);
    expect(drafts[1]?.defaultValue).toBe('guest');
    expect(drafts[1]?.comment).toBe('登录名');
  });

  it('索引草稿排除 PRIMARY', () => {
    const drafts = toIndexDrafts(schema);
    expect(drafts).toHaveLength(1);
    expect(drafts[0]?.name).toBe('idx_username');
    expect(drafts[0]?.columns).toBe('username');
  });
});
