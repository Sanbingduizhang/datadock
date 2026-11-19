import { describe, expect, it } from 'vitest';

import { formatErrorSummary, isAppError, normalizeError } from '@/utils/errors';

describe('isAppError', () => {
  it('识别符合后端错误结构的对象', () => {
    expect(isAppError({ code: 'INVALID_ARGUMENT', message: '参数无效' })).toBe(true);
  });

  it('拒绝缺少必要字段的对象', () => {
    expect(isAppError({ message: '只有消息' })).toBe(false);
    expect(isAppError({ code: 'X' })).toBe(false);
    expect(isAppError(null)).toBe(false);
    expect(isAppError('boom')).toBe(false);
  });
});

describe('normalizeError', () => {
  it('原样保留后端错误码与信息', () => {
    const result = normalizeError({
      code: 'TASK_NOT_FOUND',
      message: '任务不存在',
      details: { taskId: 't-1' },
    });

    expect(result).toEqual({
      code: 'TASK_NOT_FOUND',
      message: '任务不存在',
      details: { taskId: 't-1' },
    });
  });

  it('将 Error 实例转换为内部错误', () => {
    const result = normalizeError(new Error('连接被拒绝'));

    expect(result.code).toBe('INTERNAL_ERROR');
    expect(result.message).toBe('连接被拒绝');
  });

  it('将字符串异常转换为内部错误', () => {
    expect(normalizeError('自定义失败')).toEqual({
      code: 'INTERNAL_ERROR',
      message: '自定义失败',
    });
  });

  it('对未知异常给出兜底提示', () => {
    const result = normalizeError(undefined);

    expect(result.code).toBe('INTERNAL_ERROR');
    expect(result.message).toBe('操作失败，请稍后重试');
  });

  it('空字符串不作为有效消息', () => {
    const result = normalizeError('   ');

    expect(result.message).toBe('操作失败，请稍后重试');
  });
});

describe('formatErrorSummary', () => {
  it('拼接可读摘要', () => {
    expect(formatErrorSummary({ code: 'STORAGE_ERROR', message: '本地存储不可写' })).toBe(
      '本地存储不可写（STORAGE_ERROR）',
    );
  });
});
