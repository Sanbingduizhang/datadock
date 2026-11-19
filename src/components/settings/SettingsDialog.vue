<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppModal from '@/components/common/AppModal.vue';

import { getSettings, updateSettings } from '@/api/settings';
import { useToast } from '@/composables/useToast';
import { useAppStore } from '@/stores/app';
import { normalizeError } from '@/utils/errors';

import type { AppSettings, ThemeSetting } from '@/types/settings';

/** 设置弹窗：读写应用设置，保存后立即生效（主题、编辑器等）。 */
const props = defineProps<{ modelValue: boolean }>();

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>();

const appStore = useAppStore();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const errorMessage = ref('');
const draft = ref<AppSettings | null>(null);
/** 打开弹窗时的主题，取消时回滚预览 */
let themeSnapshot: ThemeSetting = 'system';
let saved = false;

async function load(): Promise<void> {
  loading.value = true;
  errorMessage.value = '';
  saved = false;
  themeSnapshot = appStore.settings?.appearance.theme ?? appStore.themeMode;
  try {
    draft.value = await getSettings();
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(Math.max(Math.round(value), min), max);
}

async function save(): Promise<void> {
  if (!draft.value || saving.value) {
    return;
  }

  const settings: AppSettings = {
    appearance: { theme: draft.value.appearance.theme },
    editor: {
      fontSize: clamp(draft.value.editor.fontSize, 11, 20),
      tabSize: draft.value.editor.tabSize === 4 ? 4 : 2,
      wordWrap: draft.value.editor.wordWrap === 'on' ? 'on' : 'off',
    },
    query: {
      defaultMaxRows: clamp(draft.value.query.defaultMaxRows, 100, 10_000),
    },
    data: {
      defaultPageSize: clamp(draft.value.data.defaultPageSize, 50, 1_000),
    },
    history: { enabled: draft.value.history.enabled },
    general: { restoreWorkspace: draft.value.general.restoreWorkspace },
  };

  saving.value = true;
  try {
    await updateSettings(settings);
    // 重新加载以获得后端回写的真实值（含主题合并逻辑）
    await appStore.loadSettings();
    saved = true;
    toast.success('设置已保存');
    emit('update:modelValue', false);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    saving.value = false;
  }
}

function setTheme(theme: ThemeSetting): void {
  if (!draft.value) {
    return;
  }
  draft.value.appearance.theme = theme;
  // 立即预览，取消时恢复为已保存的主题
  appStore.setThemeMode(theme);
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      void load();
      return;
    }
    // 没有保存时回滚主题预览
    if (!saved) {
      appStore.setThemeMode(themeSnapshot);
    }
  },
);

onMounted(() => {
  if (props.modelValue) {
    void load();
  }
});
</script>

<template>
  <AppModal
    :model-value="modelValue"
    title="设置"
    :width="560"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <p v-if="errorMessage" class="settings-error">{{ errorMessage }}</p>
    <p v-else-if="loading || !draft" class="settings-loading">正在读取设置…</p>

    <div v-else class="settings">
      <section class="group">
        <h3 class="group__title">外观</h3>
        <div class="field">
          <span class="field__label">主题</span>
          <div class="segmented">
            <button
              v-for="option in [
                { value: 'light', label: '浅色' },
                { value: 'dark', label: '深色' },
                { value: 'glass', label: '玻璃' },
                { value: 'skeuomorph', label: '拟物' },
                { value: 'system', label: '跟随系统' },
              ]"
              :key="option.value"
              type="button"
              class="segmented__item"
              :class="{ 'is-active': draft.appearance.theme === option.value }"
              @click="setTheme(option.value as ThemeSetting)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </section>

      <section class="group">
        <h3 class="group__title">编辑器</h3>
        <div class="field">
          <span class="field__label">字号</span>
          <input v-model.number="draft.editor.fontSize" class="field__input is-narrow" type="number" min="11" max="20" />
          <span class="field__unit">px</span>
        </div>
        <div class="field">
          <span class="field__label">缩进</span>
          <select v-model.number="draft.editor.tabSize" class="field__input is-narrow">
            <option :value="2">2 空格</option>
            <option :value="4">4 空格</option>
          </select>
        </div>
        <div class="field">
          <span class="field__label">自动换行</span>
          <label class="switch">
            <input
              type="checkbox"
              :checked="draft.editor.wordWrap === 'on'"
              @change="
                draft.editor.wordWrap = ($event.target as HTMLInputElement).checked
                  ? 'on'
                  : 'off'
              "
            />
            <span>长 SQL 自动折行</span>
          </label>
        </div>
      </section>

      <section class="group">
        <h3 class="group__title">查询与数据</h3>
        <div class="field">
          <span class="field__label">默认返回行数</span>
          <input
            v-model.number="draft.query.defaultMaxRows"
            class="field__input is-narrow"
            type="number"
            min="100"
            max="10000"
            step="100"
          />
          <span class="field__unit">行（上限 10000）</span>
        </div>
        <div class="field">
          <span class="field__label">表数据每页</span>
          <input
            v-model.number="draft.data.defaultPageSize"
            class="field__input is-narrow"
            type="number"
            min="50"
            max="1000"
            step="50"
          />
          <span class="field__unit">行（50–1000）</span>
        </div>
        <div class="field">
          <span class="field__label">查询历史</span>
          <label class="switch">
            <input v-model="draft.history.enabled" type="checkbox" />
            <span>记录每次执行（关闭后不再写入）</span>
          </label>
        </div>
      </section>

      <section class="group">
        <h3 class="group__title">通用</h3>
        <div class="field">
          <span class="field__label">启动行为</span>
          <label class="switch">
            <input v-model="draft.general.restoreWorkspace" type="checkbox" />
            <span>恢复上次的标签与面板布局</span>
          </label>
        </div>
      </section>
    </div>

    <template #footer>
      <AppButton size="sm" @click="emit('update:modelValue', false)">取消</AppButton>
      <AppButton variant="primary" size="sm" :loading="saving" :disabled="!draft" @click="save">
        保存
      </AppButton>
    </template>
  </AppModal>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-height: 55vh;
  overflow: auto;
  padding-right: var(--space-1);
}

.settings-loading,
.settings-error {
  padding: var(--space-3);
  font-size: var(--font-size-xs);
}

.settings-loading {
  color: var(--text-tertiary);
}

.settings-error {
  color: var(--danger-color);
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.group__title {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  font-weight: 600;
}

.field {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 28px;
}

.field__label {
  flex: none;
  width: 110px;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.field__input {
  height: 26px;
  padding: 0 var(--space-2);
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.field__input:focus {
  border-color: var(--primary-color);
}

.field__input.is-narrow {
  width: 90px;
}

.field__unit {
  color: var(--text-tertiary);
  font-size: 11px;
}

.segmented {
  display: inline-flex;
  padding: 2px;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.segmented__item {
  padding: 2px 10px;
  color: var(--text-secondary);
  font-size: 11px;
  border-radius: var(--radius-xs);
}

.segmented__item.is-active {
  color: var(--text-primary);
  background-color: var(--surface-bg);
  box-shadow: var(--shadow-sm);
}

.switch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-tertiary);
  font-size: 11px;
}

.switch input {
  accent-color: var(--primary-color);
}
</style>
