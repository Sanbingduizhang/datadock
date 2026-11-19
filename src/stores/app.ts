import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import { getAppInfo, ping } from '@/api/app';
import { getSettings, updateSettings } from '@/api/settings';
import type { AppError, AppInfo } from '@/types/api';
import type { AppSettings } from '@/types/settings';
import { normalizeError } from '@/utils/errors';

export type ThemeMode = 'light' | 'dark' | 'system' | 'glass' | 'skeuomorph';
export type BackendStatus = 'unknown' | 'checking' | 'ready' | 'error';

/**
 * 应用全局状态：运行环境信息、后端连通性、应用设置与主题。
 * 只放跨组件共享的状态；页面级临时状态留在组件内部。
 * 此处不保存任何数据库凭据。
 */
export const useAppStore = defineStore('app', () => {
  const appInfo = ref<AppInfo | null>(null);
  const backendStatus = ref<BackendStatus>('unknown');
  const backendLatencyMs = ref<number | null>(null);
  const lastError = ref<AppError | null>(null);
  const settings = ref<AppSettings | null>(null);

  const themeMode = ref<ThemeMode>('system');
  const systemPrefersDark = ref(false);

  /** data-theme 实际取值：玻璃/拟物是浅色变体，仅用于 CSS 选择器 */
  const resolvedTheme = computed<'light' | 'dark' | 'glass' | 'skeuomorph'>(() => {
    if (themeMode.value === 'system') {
      return systemPrefersDark.value ? 'dark' : 'light';
    }
    return themeMode.value;
  });

  /** Monaco 编辑器主题：玻璃/拟物映射到浅色（避免编辑器深色与面板不一致） */
  const monacoTheme = computed<'light' | 'dark'>(() => {
    if (resolvedTheme.value === 'dark') {
      return 'dark';
    }
    return 'light';
  });

  function applyThemeToDocument(): void {
    document.documentElement.dataset.theme = resolvedTheme.value;
  }

  /** 主题切换立即生效，同时异步持久化到本地设置 */
  function setThemeMode(mode: ThemeMode): void {
    themeMode.value = mode;
    applyThemeToDocument();

    if (settings.value) {
      settings.value.appearance.theme = mode;
    }

    void updateSettings({ appearance: { theme: mode } }).catch((error: unknown) => {
      lastError.value = normalizeError(error);
    });
  }

  /** 在当前解析结果上切换到相反主题（显式切换，脱离跟随系统） */
  function toggleTheme(): void {
    setThemeMode(monacoTheme.value === 'dark' ? 'light' : 'dark');
  }

  let systemThemeBound = false;

  /** 初始化主题：默认跟随系统，并监听系统主题变化 */
  function initTheme(): void {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    systemPrefersDark.value = media.matches;

    if (!systemThemeBound) {
      media.addEventListener('change', (event) => {
        systemPrefersDark.value = event.matches;
        applyThemeToDocument();
      });
      systemThemeBound = true;
    }

    applyThemeToDocument();
  }

  /** 检查 Rust 后端可用性，返回后端回声内容 */
  async function checkBackend(): Promise<string | null> {
    backendStatus.value = 'checking';
    lastError.value = null;

    const startedAt = performance.now();
    try {
      const response = await ping({ message: 'hello from frontend' });
      backendLatencyMs.value = Math.round(performance.now() - startedAt);
      backendStatus.value = 'ready';
      return response.reply;
    } catch (error) {
      backendLatencyMs.value = null;
      backendStatus.value = 'error';
      lastError.value = normalizeError(error);
      return null;
    }
  }

  /** 加载应用与运行环境信息 */
  async function loadAppInfo(): Promise<AppInfo | null> {
    try {
      const info = await getAppInfo();
      appInfo.value = info;
      return info;
    } catch (error) {
      lastError.value = normalizeError(error);
      return null;
    }
  }

  /** 加载应用设置并应用主题 */
  async function loadSettings(): Promise<AppSettings | null> {
    try {
      const loaded = await getSettings();
      settings.value = loaded;
      themeMode.value = loaded.appearance.theme;
      applyThemeToDocument();
      return loaded;
    } catch (error) {
      lastError.value = normalizeError(error);
      return null;
    }
  }

  return {
    appInfo,
    backendStatus,
    backendLatencyMs,
    lastError,
    settings,
    themeMode,
    resolvedTheme,
    monacoTheme,
    initTheme,
    setThemeMode,
    toggleTheme,
    checkBackend,
    loadAppInfo,
    loadSettings,
  };
});
