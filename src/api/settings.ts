import { invokeCommand } from './index';

import type {
  AppSettings,
  PartialAppSettings,
  UpdateSettingsRequest,
  UpdateSettingsResponse,
} from '@/types/settings';

/** 读取全部应用设置 */
export function getSettings(): Promise<AppSettings> {
  return invokeCommand<AppSettings>('get_all_settings');
}

/** 部分更新应用设置（后端二次校验类型与范围） */
export function updateSettings(settings: PartialAppSettings): Promise<UpdateSettingsResponse> {
  const request: UpdateSettingsRequest = { settings };
  return invokeCommand<UpdateSettingsResponse>('update_settings', { request });
}
