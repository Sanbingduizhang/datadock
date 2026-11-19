import type { AppError } from '@/types/api';

export const INTERNAL_ERROR_CODE = 'INTERNAL_ERROR';
export const DEFAULT_ERROR_MESSAGE = '操作失败，请稍后重试';

/**
 * 判断一个未知值是否符合后端统一错误结构。
 * Tauri Command 返回 Err 时，前端收到的是该结构序列化后的对象。
 */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return typeof candidate.code === 'string' && typeof candidate.message === 'string';
}

/**
 * 将任意异常归一化为 AppError，保证 UI 层只处理一种错误形状。
 * 不允许把底层堆栈直接暴露给用户。
 */
export function normalizeError(raw: unknown): AppError {
  if (isAppError(raw)) {
    const error: AppError = { code: raw.code, message: raw.message };
    if (raw.details && typeof raw.details === 'object') {
      error.details = raw.details;
    }
    return error;
  }

  if (raw instanceof Error) {
    return { code: INTERNAL_ERROR_CODE, message: raw.message || DEFAULT_ERROR_MESSAGE };
  }

  if (typeof raw === 'string' && raw.trim().length > 0) {
    return { code: INTERNAL_ERROR_CODE, message: raw };
  }

  return { code: INTERNAL_ERROR_CODE, message: DEFAULT_ERROR_MESSAGE };
}

/** 生成便于展示的错误摘要：错误码 + 可读信息 */
export function formatErrorSummary(error: AppError): string {
  return `${error.message}（${error.code}）`;
}
