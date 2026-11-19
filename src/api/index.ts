import { invoke } from '@tauri-apps/api/core';

import { normalizeError } from '@/utils/errors';

/**
 * 统一 IPC 调用入口。
 * 所有前端 API 模块都必须通过该函数调用 Rust Command，避免重复封装 invoke。
 * 失败时抛出 AppError，调用方按错误码处理。
 */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}
