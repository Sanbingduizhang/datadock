<script setup lang="ts">
import {
  CircleAlert,
  Copy,
  Eye,
  KeyRound,
  Plus,
  RefreshCw,
  Save,
  Search,
  Trash2,
} from 'lucide-vue-next';
import { computed, onMounted, ref } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppModal from '@/components/common/AppModal.vue';
import ResultGrid from '@/components/query/ResultGrid.vue';

import {
  redisDeleteKey,
  redisGetKeyValue,
  redisScanKeys,
  redisSetKeyValue,
  redisSetTtl,
} from '@/api/redis';
import { useToast } from '@/composables/useToast';
import { copyText } from '@/utils/clipboard';
import { normalizeError } from '@/utils/errors';

import type {
  HashField,
  ListItem,
  RedisKeyInfo,
  RedisKeyType,
  RedisKeyValue,
  ZsetEntry,
} from '@/types/redis';
import type { WorkspaceTab } from '@/types/workspace';

/**
 * Redis 库浏览器（右侧主区域）：键列表 + 值详情，类似 TablePlus。
 *
 * - 左栏键列表（支持模式搜索），右栏值详情；
 * - 支持查看 string / hash / list / set / zset 值；
 * - 支持编辑值、新增键、删除键、设置 TTL。
 */
const props = defineProps<{ tab: WorkspaceTab }>();

const toast = useToast();

const connectionId = computed(() => props.tab.context.connectionId ?? '');
const dbIndex = computed(() => props.tab.context.redisDbIndex ?? 0);

const loading = ref(false);
const errorMessage = ref('');
const keys = ref<RedisKeyInfo[]>([]);

/** 键模式过滤 */
const filterText = ref('');

/** 选中查看的键 */
const selectedKey = ref<RedisKeyInfo | null>(null);
const detailLoading = ref(false);
const detailError = ref('');
const detailType = ref<RedisKeyType>('none');
const detailTtl = ref<number>(-1);
const detailValue = ref<RedisKeyValue | null>(null);

/** 新增键弹窗 */
const createVisible = ref(false);
const createName = ref('');
const createType = ref<RedisKeyType>('string');
const createTtl = ref<number>(0);

const TYPE_OPTIONS: { value: RedisKeyType; label: string }[] = [
  { value: 'string', label: 'string' },
  { value: 'hash', label: 'hash' },
  { value: 'list', label: 'list' },
  { value: 'set', label: 'set' },
  { value: 'zset', label: 'zset' },
  { value: 'stream', label: 'stream' },
];

/* ---------- 键列表 ---------- */

const filteredKeys = computed<RedisKeyInfo[]>(() => {
  const keyword = filterText.value.trim().toLowerCase();
  if (!keyword) {
    return keys.value;
  }
  return keys.value.filter((key) => key.name.toLowerCase().includes(keyword));
});

async function loadKeys(): Promise<void> {
  if (!connectionId.value) {
    errorMessage.value = '标签上下文不完整，请重新打开库';
    return;
  }

  loading.value = true;
  errorMessage.value = '';

  try {
    const response = await redisScanKeys({
      connectionId: connectionId.value,
      database: dbIndex.value,
      pattern: '*',
      count: 500,
      includeSystemKeys: false,
    });
    keys.value = response.keys;
  } catch (raw) {
    keys.value = [];
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

/* ---------- 值详情 ---------- */

function formatTtl(ttl: number): string {
  if (ttl === -1) return '永不过期';
  if (ttl === -2) return '已过期';
  if (ttl < 60) return `${ttl}s`;
  if (ttl < 3600) return `${Math.floor(ttl / 60)}m`;
  if (ttl < 86400) return `${Math.floor(ttl / 3600)}h`;
  return `${Math.floor(ttl / 86400)}d`;
}

async function selectKey(key: RedisKeyInfo): Promise<void> {
  selectedKey.value = key;
  await loadDetail();
}

async function loadDetail(): Promise<void> {
  const key = selectedKey.value;
  if (!key || !connectionId.value) {
    return;
  }

  detailLoading.value = true;
  detailError.value = '';

  try {
    const response = await redisGetKeyValue({
      connectionId: connectionId.value,
      database: dbIndex.value,
      keyName: key.name,
      maxItems: 500,
    });
    detailType.value = response.type;
    detailTtl.value = response.ttlSeconds;
    detailValue.value = response.value;
  } catch (raw) {
    detailValue.value = null;
    detailError.value = normalizeError(raw).message;
  } finally {
    detailLoading.value = false;
  }
}

/* ---------- 编辑 ---------- */

const stringValue = computed<string>({
  get: () =>
    detailValue.value?.kind === 'string' ? (detailValue.value.value ?? '') : '',
  set: (value: string) => {
    if (detailValue.value?.kind === 'string') {
      detailValue.value = { kind: 'string', value };
    }
  },
});

const hashFields = computed<HashField[]>(() =>
  detailValue.value?.kind === 'hash' ? detailValue.value.fields : [],
);

const listItems = computed<ListItem[]>(() =>
  detailValue.value?.kind === 'list' ? detailValue.value.items : [],
);

const setMembers = computed<string[]>(() =>
  detailValue.value?.kind === 'set' ? detailValue.value.members : [],
);

const zsetEntries = computed<ZsetEntry[]>(() =>
  detailValue.value?.kind === 'zset' ? detailValue.value.entries : [],
);

const editing = ref(false);

async function saveValue(): Promise<void> {
  const key = selectedKey.value;
  if (!key || detailValue.value === null) {
    return;
  }

  try {
    await redisSetKeyValue(
      connectionId.value,
      dbIndex.value,
      key.name,
      detailType.value,
      detailValue.value,
    );
    toast.success(`已保存 ${key.name}`);
    editing.value = false;
    await loadDetail();
    await loadKeys();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

async function saveTtl(ttl: number): Promise<void> {
  const key = selectedKey.value;
  if (!key) {
    return;
  }

  try {
    await redisSetTtl(connectionId.value, dbIndex.value, key.name, ttl);
    toast.success(ttl > 0 ? `已设置 ${formatTtl(ttl)} 过期` : '已移除过期');
    await loadDetail();
    await loadKeys();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

async function removeKey(key: RedisKeyInfo): Promise<void> {
  try {
    await redisDeleteKey(connectionId.value, dbIndex.value, key.name);
    toast.success(`已删除 ${key.name}`);
    if (selectedKey.value?.name === key.name) {
      selectedKey.value = null;
      detailValue.value = null;
    }
    await loadKeys();
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

/* ---------- 新增 ---------- */

function openCreateDialog(): void {
  createName.value = '';
  createType.value = 'string';
  createTtl.value = 0;
  createVisible.value = true;
}

async function createKey(): Promise<void> {
  const name = createName.value.trim();
  if (!name) {
    toast.warning('请输入键名');
    return;
  }

  // 根据类型提供初始空值
  const initialValue: RedisKeyValue = (() => {
    switch (createType.value) {
      case 'string':
        return { kind: 'string', value: '' };
      case 'hash':
        return { kind: 'hash', fields: [] };
      case 'list':
        return { kind: 'list', items: [] };
      case 'set':
        return { kind: 'set', members: [] };
      case 'zset':
        return { kind: 'zset', entries: [] };
      case 'stream':
        return { kind: 'stream', entries: [] };
      default:
        return { kind: 'string', value: '' };
    }
  })();

  try {
    await redisSetKeyValue(
      connectionId.value,
      dbIndex.value,
      name,
      createType.value,
      initialValue,
      createTtl.value > 0 ? createTtl.value : undefined,
    );
    toast.success(`已创建 ${name}`);
    createVisible.value = false;
    await loadKeys();

    // 选中新创建的键
    const newKey = keys.value.find((key) => key.name === name);
    if (newKey) {
      await selectKey(newKey);
    }
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

async function copyKeyName(name: string): Promise<void> {
  if (await copyText(name)) {
    toast.success(`已复制「${name}」`);
  } else {
    toast.error('复制失败，请手动选择文本复制');
  }
}

onMounted(() => {
  void loadKeys();
});
</script>

<template>
  <section class="redis-db">
    <header class="redis-db__bar">
      <span class="redis-db__title">
        <KeyRound :size="14" />
        db{{ dbIndex }}
      </span>
      <span class="redis-db__meta">{{ filteredKeys.length }} 个键</span>
      <span class="redis-db__spacer"></span>

      <AppButton variant="ghost" size="sm" :disabled="loading" @click="loadKeys">
        <RefreshCw :size="13" />
        刷新
      </AppButton>
      <AppButton variant="primary" size="sm" @click="openCreateDialog">
        <Plus :size="13" />
        新增键
      </AppButton>
    </header>

    <div v-if="errorMessage" class="redis-db__error">
      <CircleAlert :size="14" />
      <span>{{ errorMessage }}</span>
    </div>

    <div v-else-if="loading && keys.length === 0" class="redis-db__hint">正在扫描键…</div>

    <div v-else class="redis-db__body">
      <!-- 左栏：键列表 -->
      <aside class="redis-db__keys">
        <div class="redis-db__filter">
          <Search :size="13" />
          <input
            v-model="filterText"
            type="text"
            placeholder="筛选键名"
            spellcheck="false"
            autocomplete="off"
          />
        </div>

        <div class="redis-db__key-list">
          <p v-if="filteredKeys.length === 0" class="redis-db__hint">没有匹配的键</p>
          <div
            v-for="key in filteredKeys"
            :key="key.name"
            class="redis-db__key"
            :class="{ 'is-selected': selectedKey?.name === key.name }"
            @click="selectKey(key)"
          >
            <component
              :is="key.type === 'string' ? Eye : KeyRound"
              :size="12"
              class="redis-db__key-icon"
            />
            <span class="redis-db__key-name">{{ key.name }}</span>
            <span class="redis-db__key-type">{{ key.type }}</span>
            <span v-if="key.ttlSeconds >= 0" class="redis-db__key-ttl">
              {{ formatTtl(key.ttlSeconds) }}
            </span>
            <button
              type="button"
              class="redis-db__key-action"
              title="删除键"
              @click.stop="removeKey(key)"
            >
              <Trash2 :size="12" />
            </button>
          </div>
        </div>
      </aside>

      <!-- 右栏：值详情 -->
      <div class="redis-db__detail">
        <template v-if="selectedKey">
          <header class="redis-db__detail-bar">
            <span class="redis-db__detail-name">
              <KeyRound :size="13" />
              {{ selectedKey.name }}
            </span>
            <span class="redis-db__detail-type">{{ detailType }}</span>
            <span v-if="detailTtl >= 0" class="redis-db__detail-ttl">
              {{ formatTtl(detailTtl) }}
            </span>
            <span class="redis-db__detail-spacer"></span>

            <AppButton
              v-if="detailTtl === -1"
              variant="ghost"
              size="sm"
              title="设置 1 小时过期"
              @click="saveTtl(3600)"
            >
              设置过期
            </AppButton>
            <AppButton
              v-else
              variant="ghost"
              size="sm"
              title="移除过期"
              @click="saveTtl(0)"
            >
              移除过期
            </AppButton>
            <AppButton
              variant="ghost"
              size="sm"
              title="复制键名"
              @click="copyKeyName(selectedKey.name)"
            >
              <Copy :size="13" />
            </AppButton>
            <AppButton
              v-if="!editing"
              variant="primary"
              size="sm"
              @click="editing = true"
            >
              编辑
            </AppButton>
            <template v-else>
              <AppButton variant="ghost" size="sm" @click="editing = false">取消</AppButton>
              <AppButton variant="primary" size="sm" @click="saveValue">
                <Save :size="13" />
                保存
              </AppButton>
            </template>
          </header>

          <div v-if="detailError" class="redis-db__detail-error">{{ detailError }}</div>
          <p v-else-if="detailLoading" class="redis-db__hint">正在读取值…</p>

          <!-- string -->
          <template v-else-if="detailValue?.kind === 'string'">
            <textarea
              v-if="editing"
              v-model="stringValue"
              class="redis-db__value-editor"
              spellcheck="false"
            ></textarea>
            <pre v-else class="redis-db__value-pre">{{ stringValue || '(空)' }}</pre>
          </template>

          <!-- hash -->
          <template v-else-if="detailValue?.kind === 'hash'">
            <ResultGrid
              v-if="hashFields.length > 0"
              :set="{
                columns: [
                  { name: '字段', typeName: 'string' },
                  { name: '值', typeName: 'string' },
                ],
                rows: hashFields.map((field) => [
                  { type: 'string', value: field.field },
                  { type: 'string', value: field.value },
                ]),
                rowCount: hashFields.length,
                truncated: false,
              }"
            />
            <p v-else class="redis-db__hint">空 Hash</p>
          </template>

          <!-- list -->
          <template v-else-if="detailValue?.kind === 'list'">
            <ResultGrid
              v-if="listItems.length > 0"
              :set="{
                columns: [
                  { name: '索引', typeName: 'integer' },
                  { name: '值', typeName: 'string' },
                ],
                rows: listItems.map((item) => [
                  { type: 'integer', value: item.index.toString() },
                  { type: 'string', value: item.value },
                ]),
                rowCount: listItems.length,
                truncated: false,
              }"
            />
            <p v-else class="redis-db__hint">空 List</p>
          </template>

          <!-- set -->
          <template v-else-if="detailValue?.kind === 'set'">
            <ResultGrid
              v-if="setMembers.length > 0"
              :set="{
                columns: [{ name: '成员', typeName: 'string' }],
                rows: setMembers.map((member) => [{ type: 'string', value: member }]),
                rowCount: setMembers.length,
                truncated: false,
              }"
            />
            <p v-else class="redis-db__hint">空 Set</p>
          </template>

          <!-- zset -->
          <template v-else-if="detailValue?.kind === 'zset'">
            <ResultGrid
              v-if="zsetEntries.length > 0"
              :set="{
                columns: [
                  { name: '成员', typeName: 'string' },
                  { name: '分数', typeName: 'double' },
                ],
                rows: zsetEntries.map((entry) => [
                  { type: 'string', value: entry.member },
                  { type: 'double', value: entry.score.toString() },
                ]),
                rowCount: zsetEntries.length,
                truncated: false,
              }"
            />
            <p v-else class="redis-db__hint">空 ZSet</p>
          </template>

          <p v-else class="redis-db__hint">键不存在或类型不支持</p>
        </template>

        <div v-else class="redis-db__empty">
          <KeyRound :size="32" />
          <p>选择左侧键查看值</p>
        </div>
      </div>
    </div>

    <!-- 新增键弹窗 -->
    <AppModal v-model="createVisible" title="新增 Redis 键" :width="480">
      <div class="redis-db__form">
        <label class="redis-db__form-field">
          <span class="redis-db__form-label">键名</span>
          <input v-model="createName" type="text" placeholder="user:1" spellcheck="false" autocomplete="off" />
        </label>
        <label class="redis-db__form-field">
          <span class="redis-db__form-label">类型</span>
          <select v-model="createType">
            <option v-for="option in TYPE_OPTIONS" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>
        <label class="redis-db__form-field">
          <span class="redis-db__form-label">过期时间（秒）</span>
          <input v-model.number="createTtl" type="number" min="0" placeholder="0 = 永不过期" />
        </label>
      </div>
      <template #footer>
        <AppButton size="sm" @click="createVisible = false">取消</AppButton>
        <AppButton variant="primary" size="sm" :disabled="!createName.trim()" @click="createKey">
          创建
        </AppButton>
      </template>
    </AppModal>
  </section>
</template>

<style scoped>
.redis-db {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--surface-bg);
}

.redis-db__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.redis-db__title {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 500;
}

.redis-db__meta {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.redis-db__spacer,
.redis-db__detail-spacer {
  flex: 1;
}

.redis-db__error,
.redis-db__hint {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.redis-db__error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  color: var(--danger-color);
  font-family: var(--font-mono);
}

.redis-db__body {
  display: flex;
  flex: 1;
  min-height: 0;
}

/* 左栏键列表 */
.redis-db__keys {
  display: flex;
  flex-direction: column;
  flex: none;
  width: 280px;
  min-height: 0;
  border-right: 1px solid var(--divider-color);
}

.redis-db__filter {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 var(--space-2);
  color: var(--text-tertiary);
  border-bottom: 1px solid var(--divider-color);
}

.redis-db__filter input {
  flex: 1;
  min-width: 0;
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  background-color: transparent;
  border: none;
  outline: none;
}

.redis-db__key-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.redis-db__key {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  cursor: default;
  user-select: none;
}

.redis-db__key:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.redis-db__key.is-selected {
  color: var(--text-primary);
  background-color: var(--surface-selected);
}

.redis-db__key-icon {
  flex: none;
  color: var(--text-tertiary);
}

.redis-db__key-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.redis-db__key-type,
.redis-db__key-ttl {
  flex: none;
  color: var(--text-tertiary);
  font-size: 10px;
  font-family: var(--font-mono);
}

.redis-db__key-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 18px;
  height: 18px;
  color: var(--text-tertiary);
  border-radius: var(--radius-xs);
  opacity: 0;
}

.redis-db__key:hover .redis-db__key-action {
  opacity: 1;
}

.redis-db__key-action:hover {
  color: var(--danger-color);
  background-color: var(--danger-soft);
}

/* 右栏值详情 */
.redis-db__detail {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
}

.redis-db__detail-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 32px;
  padding: 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  border-bottom: 1px solid var(--divider-color);
}

.redis-db__detail-name {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.redis-db__detail-type {
  flex: none;
  padding: 1px 6px;
  color: var(--text-tertiary);
  font-size: 10px;
  background-color: var(--surface-subtle);
  border-radius: var(--radius-xs);
}

.redis-db__detail-ttl {
  flex: none;
  color: var(--text-tertiary);
  font-size: 10px;
}

.redis-db__detail-error {
  padding: var(--space-3);
  color: var(--danger-color);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

.redis-db__value-editor {
  flex: 1;
  min-height: 0;
  padding: var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  background-color: var(--surface-subtle);
  border: 1px solid var(--primary-color);
  border-radius: var(--radius-sm);
  outline: none;
  resize: none;
}

.redis-db__value-pre {
  flex: 1;
  min-height: 0;
  padding: var(--space-3);
  overflow: auto;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
  background-color: var(--surface-subtle);
  user-select: text;
}

.redis-db__empty {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

/* 新增键表单 */
.redis-db__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.redis-db__form-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.redis-db__form-label {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.redis-db__form-field input,
.redis-db__form-field select {
  height: 30px;
  padding: 0 var(--space-2);
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.redis-db__form-field input:focus,
.redis-db__form-field select:focus {
  border-color: var(--primary-color);
}
</style>
