import { invokeCommand } from './index';

import type { AppInfo, PingRequest, PingResponse } from '@/types/api';

/** 后端连通性检查（用于状态栏刷新与错误提示） */
export function ping(params: PingRequest = {}): Promise<PingResponse> {
  return invokeCommand<PingResponse>('ping', { request: params });
}

/** 获取应用与运行环境基础信息 */
export function getAppInfo(): Promise<AppInfo> {
  return invokeCommand<AppInfo>('get_app_info');
}
