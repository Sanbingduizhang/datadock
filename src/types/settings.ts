/** 应用设置类型，与 src-tauri/src/models/settings.rs 保持一致 */

export type ThemeSetting = 'light' | 'dark' | 'system' | 'glass' | 'skeuomorph';

export type WordWrapSetting = 'on' | 'off';

export interface AppearanceSettings {
  theme: ThemeSetting;
}

export interface EditorSettings {
  fontSize: number;
  tabSize: number;
  wordWrap: WordWrapSetting;
}

export interface QuerySettings {
  defaultMaxRows: number;
}

export interface DataSettings {
  defaultPageSize: number;
}

export interface HistorySettings {
  enabled: boolean;
}

export interface GeneralSettings {
  restoreWorkspace: boolean;
}

export interface AppSettings {
  appearance: AppearanceSettings;
  editor: EditorSettings;
  query: QuerySettings;
  data: DataSettings;
  history: HistorySettings;
  general: GeneralSettings;
}

/** 部分更新：只传需要修改的字段，后端会做范围校验 */
export interface PartialAppSettings {
  appearance?: Partial<AppearanceSettings>;
  editor?: Partial<EditorSettings>;
  query?: Partial<QuerySettings>;
  data?: Partial<DataSettings>;
  history?: Partial<HistorySettings>;
  general?: Partial<GeneralSettings>;
}

export interface UpdateSettingsRequest {
  settings: PartialAppSettings;
}

export interface UpdateSettingsResponse {
  success: boolean;
}
