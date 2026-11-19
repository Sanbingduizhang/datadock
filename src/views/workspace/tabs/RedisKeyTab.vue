<script setup lang="ts">
import {
  CircleAlert,
  Clock,
  KeyRound,
  RefreshCw,
} from 'lucide-vue-next';
import { computed, onMounted, ref } from 'vue';

import AppButton from '@/components/common/AppButton.vue';

import { redisGetKeyValue } from '@/api/redis';
import { normalizeError } from '@/utils/errors';

import type {
  HashField,
  ListItem,
  RedisKeyValueResponse,
  ZsetEntry,
} from '@/types/redis';
import type { WorkspaceTab } from '@/types/workspace';

/**
 * Redis 键值查看器：按类型（string / hash / list / set / zset / stream）渲染键值。
 * 仅读取，不支持编辑；TTL 实时展示。
 */
const props = defineProps<{ tab: WorkspaceTab }>();

const loading = ref(false);
const errorMessage = ref('');
const data = ref<RedisKeyValueResponse | null>(null);

const canLoad = computed(
  () => Boolean(props.tab.context.connectionId && props.tab.context.keyName),
);

const keyName = computed(() => props.tab.context.keyName ?? '');
const connectionId = computed(() => props.tab.context.connectionId ?? '');
/** Redis 库索引（打开键时所在库），缺省回退 0 */
const dbIndex = computed(() => props.tab.context.redisDbIndex ?? 0);

/** 格式化 TTL 显示 */
const ttlLabel = computed(() => {
  const ttl = data.value?.ttlSeconds;
  if (ttl === undefined) return '';
  if (ttl === -1) return '永不过期';
  if (ttl === -2) return '';
  if (ttl < 60) return `${ttl}s`;
  if (ttl < 3600) return `${Math.floor(ttl / 60)}m`;
  if (ttl < 86400) return `${Math.floor(ttl / 3600)}h`;
  return `${Math.floor(ttl / 86400)}d`;
});

const keyValue = computed(() => data.value?.value);

/* ---------- 按类型渲染辅助 ---------- */

const stringValue = computed(() =>
  keyValue.value?.kind === 'string' ? keyValue.value.value : null,
);

const hashFields = computed<HashField[]>(() =>
  keyValue.value?.kind === 'hash' ? keyValue.value.fields : [],
);

const listItems = computed<ListItem[]>(() =>
  keyValue.value?.kind === 'list' ? keyValue.value.items : [],
);

const setMembers = computed<string[]>(() =>
  keyValue.value?.kind === 'set' ? keyValue.value.members : [],
);

const zsetEntries = computed<ZsetEntry[]>(() =>
  keyValue.value?.kind === 'zset' ? keyValue.value.entries : [],
);

const isHash = computed(() => keyValue.value?.kind === 'hash');
const isList = computed(() => keyValue.value?.kind === 'list');
const isSet = computed(() => keyValue.value?.kind === 'set');
const isZset = computed(() => keyValue.value?.kind === 'zset');
const isString = computed(() => keyValue.value?.kind === 'string');
const isNone = computed(() => keyValue.value?.kind === 'none' || keyValue.value === undefined);

async function load(): Promise<void> {
  if (!connectionId.value || !keyName.value) {
    errorMessage.value = '标签上下文不完整，请重新打开键';
    return;
  }

  loading.value = true;
  errorMessage.value = '';

  try {
    data.value = await redisGetKeyValue({
      connectionId: connectionId.value,
      database: dbIndex.value,
      keyName: keyName.value,
      maxItems: 200,
    });
  } catch (raw) {
    data.value = null;
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  if (canLoad.value) {
    void load();
  }
});
</script>

<template>
  <section class="redis-key-tab">
    <header class="redis-key-tab__bar">
      <span class="redis-key-tab__title">
        <KeyRound :size="13" />
        {{ keyName }}
      </span>
      <span v-if="data" class="redis-key-tab__meta">{{ data.type }}</span>
      <span v-if="ttlLabel" class="redis-key-tab__ttl">
        <Clock :size="12" />
        {{ ttlLabel }}
      </span>
      <span class="redis-key-tab__spacer"></span>
      <AppButton variant="ghost" size="sm" :disabled="loading" title="刷新值" @click="load">
        <RefreshCw :size="13" :class="{ 'is-spinning': loading }" />
        刷新
      </AppButton>
    </header>

    <div class="redis-key-tab__body" :class="{ 'is-loading': loading }">
      <div v-if="errorMessage" class="redis-key-tab__error">
        <CircleAlert :size="14" />
        <span>{{ errorMessage }}</span>
      </div>

      <p v-else-if="loading" class="redis-key-tab__hint">正在读取值…</p>

      <template v-else-if="isString">
        <div v-if="stringValue === null" class="redis-key-tab__hint">
          值为 nil
        </div>
        <pre v-else class="redis-key-tab__value">{{ stringValue }}</pre>
      </template>

      <template v-else-if="isHash">
        <div v-if="hashFields.length === 0" class="redis-key-tab__hint">空 Hash</div>
        <table v-else class="redis-key-tab__table">
          <thead>
            <tr><th>字段</th><th>值</th></tr>
          </thead>
          <tbody>
            <tr v-for="field in hashFields" :key="field.field">
              <td class="redis-key-tab__key-cell">{{ field.field }}</td>
              <td class="redis-key-tab__value-cell">{{ field.value }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <template v-else-if="isList">
        <div v-if="listItems.length === 0" class="redis-key-tab__hint">空 List</div>
        <table v-else class="redis-key-tab__table">
          <thead>
            <tr><th>索引</th><th>值</th></tr>
          </thead>
          <tbody>
            <tr v-for="item in listItems" :key="item.index">
              <td class="redis-key-tab__key-cell">{{ item.index }}</td>
              <td class="redis-key-tab__value-cell">{{ item.value }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <template v-else-if="isSet">
        <div v-if="setMembers.length === 0" class="redis-key-tab__hint">空 Set</div>
        <table v-else class="redis-key-tab__table">
          <thead><tr><th>成员</th></tr></thead>
          <tbody>
            <tr v-for="member in setMembers" :key="member">
              <td class="redis-key-tab__value-cell">{{ member }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <template v-else-if="isZset">
        <div v-if="zsetEntries.length === 0" class="redis-key-tab__hint">空 ZSet</div>
        <table v-else class="redis-key-tab__table">
          <thead>
            <tr><th>成员</th><th>分数</th></tr>
          </thead>
          <tbody>
            <tr v-for="entry in zsetEntries" :key="entry.member">
              <td class="redis-key-tab__value-cell">{{ entry.member }}</td>
              <td class="redis-key-tab__key-cell">{{ entry.score }}</td>
            </tr>
          </tbody>
        </table>
      </template>

      <p v-else-if="isNone" class="redis-key-tab__hint">键不存在或类型不支持</p>
    </div>
  </section>
</template>

<style scoped>
.redis-key-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background-color: var(--surface-bg);
}

.redis-key-tab__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.redis-key-tab__title {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.redis-key-tab__meta {
  flex: none;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
  padding: 1px 6px;
  background-color: var(--surface-subtle);
  border-radius: var(--radius-xs);
}

.redis-key-tab__ttl {
  display: flex;
  align-items: center;
  gap: 3px;
  color: var(--text-tertiary);
  font-size: 11px;
}

.redis-key-tab__spacer {
  flex: 1;
}

.redis-key-tab__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-3);
}

.redis-key-tab__body.is-loading {
  cursor: progress;
}

.redis-key-tab__error {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: var(--space-3);
  color: var(--danger-color);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  user-select: text;
}

.redis-key-tab__hint {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.redis-key-tab__value {
  padding: var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  user-select: text;
}

.redis-key-tab__table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-xs);
}

.redis-key-tab__table th,
.redis-key-tab__table td {
  padding: 4px var(--space-2);
  border-bottom: 1px solid var(--divider-color);
  text-align: left;
  vertical-align: top;
}

.redis-key-tab__table th {
  color: var(--text-tertiary);
  font-weight: 500;
  font-size: 10px;
  background-color: var(--surface-subtle);
  position: sticky;
  top: 0;
}

.redis-key-tab__key-cell {
  color: var(--text-secondary);
  font-family: var(--font-mono);
  word-break: break-all;
}

.redis-key-tab__value-cell {
  color: var(--text-primary);
  font-family: var(--font-mono);
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
}

.is-spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .is-spinning {
    animation: none;
  }
}
</style>
