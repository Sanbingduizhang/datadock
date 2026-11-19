<script setup lang="ts">
import { LoaderCircle } from 'lucide-vue-next';
import { computed, ref, watch } from 'vue';

import { open, save } from '@tauri-apps/plugin-dialog';

import AppButton from '@/components/common/AppButton.vue';
import AppModal from '@/components/common/AppModal.vue';

import { dumpDatabase, importDatabase } from '@/api/export';
import { useConnectionStore } from '@/stores/connection';
import { useExplorerStore } from '@/stores/explorer';
import { useToast } from '@/composables/useToast';
import { normalizeError } from '@/utils/errors';

import type { DumpMode } from '@/types/export';

/**
 * 数据库导入导出对话框：结构 / 数据 / 结构+数据三种模式。
 * 导出为 SQL 脚本落盘；导入从 SQL 文件整体一个事务执行，失败回滚。
 */
const props = defineProps<{
  modelValue: boolean;
  /** import 或 export */
  mode: 'import' | 'export';
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
}>();

const connectionStore = useConnectionStore();
const explorerStore = useExplorerStore();
const toast = useToast();

const connectionId = ref('');
const databaseName = ref('');
const dumpMode = ref<DumpMode>('both');
const tables = ref<string[]>([]);
const busy = ref(false);

const connectedConnections = computed(() =>
  connectionStore.connections.filter((item) => item.connected),
);

const databases = computed(() => {
  if (!connectionId.value) return [];
  return explorerStore.databasesState(connectionId.value).items;
});

const tableList = computed(() => {
  if (!connectionId.value || !databaseName.value) return [];
  return explorerStore.tablesState(connectionId.value, databaseName.value).items;
});

const canSubmit = computed(() => {
  if (!connectionId.value || !databaseName.value) return false;
  if (props.mode === 'import') return true;
  return true;
});

watch(
  () => props.modelValue,
  (visible) => {
    if (visible) {
      const connected = connectedConnections.value[0];
      connectionId.value = connected?.id ?? '';
      databaseName.value = '';
      tables.value = [];
      dumpMode.value = 'both';
    }
  },
);

watch(connectionId, async (id) => {
  databaseName.value = '';
  tables.value = [];
  if (id) {
    await explorerStore.loadDatabases(id);
  }
});

watch(databaseName, async (db) => {
  tables.value = [];
  if (db && connectionId.value) {
    await explorerStore.loadTables(connectionId.value, db);
  }
});

function toggleTable(name: string): void {
  tables.value = tables.value.includes(name)
    ? tables.value.filter((item) => item !== name)
    : [...tables.value, name];
}

async function handleExport(): Promise<void> {
  const path = await save({
    title: '导出数据库',
    defaultPath: `${databaseName.value}.sql`,
    filters: [{ name: 'SQL 文件', extensions: ['sql'] }],
  });
  if (!path) return;

  busy.value = true;
  try {
    const response = await dumpDatabase({
      connectionId: connectionId.value,
      databaseName: databaseName.value,
      tables: tables.value.length > 0 ? tables.value : undefined,
      mode: dumpMode.value,
      path,
    });
    toast.success(
      `已导出 ${response.tables} 张表（${response.rowsWritten} 行）到 ${response.path}`,
    );
    emit('update:modelValue', false);
  } catch (raw) {
    toast.error(`导出失败：${normalizeError(raw).message}`);
  } finally {
    busy.value = false;
  }
}

async function handleImport(): Promise<void> {
  const path = await open({
    title: '导入数据库',
    multiple: false,
    filters: [{ name: 'SQL 文件', extensions: ['sql'] }],
  });
  if (!path || Array.isArray(path)) return;

  busy.value = true;
  try {
    const response = await importDatabase({
      connectionId: connectionId.value,
      databaseName: databaseName.value || undefined,
      path,
    });
    toast.success(`已执行 ${response.statements} 条语句`);
    emit('update:modelValue', false);
  } catch (raw) {
    toast.error(`导入失败：${normalizeError(raw).message}`);
  } finally {
    busy.value = false;
  }
}

async function handleSubmit(): Promise<void> {
  if (props.mode === 'export') {
    await handleExport();
  } else {
    await handleImport();
  }
}
</script>

<template>
  <AppModal
    :model-value="modelValue"
    :title="mode === 'export' ? '导出数据库' : '导入数据库'"
    :width="520"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <div class="dump-form">
      <label class="field">
        <span class="field__label">连接</span>
        <select v-model="connectionId" class="field__select">
          <option v-for="conn in connectedConnections" :key="conn.id" :value="conn.id">
            {{ conn.name }}（{{ conn.sourceType }}）
          </option>
        </select>
      </label>

      <label class="field">
        <span class="field__label">数据库</span>
        <select v-model="databaseName" class="field__select">
          <option value="">请选择</option>
          <option v-for="db in databases" :key="db.name" :value="db.name">
            {{ db.name }}
          </option>
        </select>
      </label>

      <div v-if="mode === 'export'" class="field">
        <span class="field__label">导出内容</span>
        <div class="mode-group">
          <label class="mode-option">
            <input v-model="dumpMode" type="radio" value="structure" />
            <span>仅结构</span>
          </label>
          <label class="mode-option">
            <input v-model="dumpMode" type="radio" value="data" />
            <span>仅数据</span>
          </label>
          <label class="mode-option">
            <input v-model="dumpMode" type="radio" value="both" />
            <span>结构 + 数据</span>
          </label>
        </div>
      </div>

      <div v-if="mode === 'export' && tableList.length > 0" class="field">
        <span class="field__label">
          选择表（不选则导出全部）
          <button class="link-btn" type="button" @click="tables = []">清空选择</button>
        </span>
        <div class="table-list">
          <label v-for="table in tableList" :key="table.name" class="table-item">
            <input
              type="checkbox"
              :checked="tables.includes(table.name)"
              @change="toggleTable(table.name)"
            />
            <span>{{ table.name }}</span>
          </label>
        </div>
      </div>

      <p v-if="mode === 'import'" class="hint">
        导入将整体一个事务执行 SQL 脚本，任一语句失败则回滚。
      </p>
    </div>

    <template #footer>
      <AppButton :disabled="busy" @click="emit('update:modelValue', false)">取消</AppButton>
      <AppButton variant="primary" :loading="busy" :disabled="!canSubmit" @click="handleSubmit">
        <LoaderCircle v-if="busy" :size="13" class="spin" />
        {{ mode === 'export' ? '导出' : '导入' }}
      </AppButton>
    </template>
  </AppModal>
</template>

<style scoped>
.dump-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.field__label {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.field__select {
  height: 30px;
  padding: 0 6px;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.mode-group {
  display: flex;
  gap: var(--space-3);
}

.mode-option {
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  font-size: var(--font-size-sm);
  color: var(--text-primary);
}

.mode-option input {
  accent-color: var(--primary-color);
  margin: 0;
}

.table-list {
  max-height: 180px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: var(--space-2);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.table-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--font-size-sm);
  color: var(--text-primary);
  cursor: pointer;
}

.table-item input {
  accent-color: var(--primary-color);
  margin: 0;
}

.link-btn {
  color: var(--primary-color);
  font-size: var(--font-size-xs);
  cursor: pointer;
  background: none;
  border: none;
  padding: 0;
}

.hint {
  color: var(--text-tertiary);
  font-size: 11px;
  line-height: 1.6;
}

.spin {
  animation: spin 900ms linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
