<script setup lang="ts">
import { Star, Trash2 } from 'lucide-vue-next';
import { computed, onMounted, ref, watch } from 'vue';

import AppEmpty from '@/components/common/AppEmpty.vue';

import { deleteSqlFavorite, listSqlFavorites } from '@/api/favorite';
import { useToast } from '@/composables/useToast';
import { useUiStore } from '@/stores/ui';
import { normalizeError } from '@/utils/errors';
import { compactSql, formatRelativeTime } from '@/utils/format';

import type { SqlFavorite } from '@/types/favorite';

/** 侧栏「收藏」面板：保存常用 SQL，点击回填到新的查询标签。 */
const props = defineProps<{ keyword: string; active: boolean }>();

const emit = defineEmits<{
  'open-sql': [payload: { sql: string; connectionId?: string; databaseName?: string }];
}>();

const uiStore = useUiStore();
const toast = useToast();
const loading = ref(false);
const errorMessage = ref('');
const favorites = ref<SqlFavorite[]>([]);

const filtered = computed(() => {
  const keyword = props.keyword.trim().toLowerCase();
  if (!keyword) {
    return favorites.value;
  }
  return favorites.value.filter(
    (item) =>
      item.name.toLowerCase().includes(keyword) || item.sqlText.toLowerCase().includes(keyword),
  );
});

async function load(): Promise<void> {
  loading.value = true;
  errorMessage.value = '';
  try {
    favorites.value = await listSqlFavorites();
  } catch (raw) {
    errorMessage.value = normalizeError(raw).message;
  } finally {
    loading.value = false;
  }
}

function open(item: SqlFavorite): void {
  emit('open-sql', {
    sql: item.sqlText,
    ...(item.connectionId ? { connectionId: item.connectionId } : {}),
    ...(item.databaseName ? { databaseName: item.databaseName } : {}),
  });
}

async function remove(item: SqlFavorite): Promise<void> {
  try {
    await deleteSqlFavorite(item.id);
    favorites.value = favorites.value.filter((entry) => entry.id !== item.id);
    toast.success(`已删除收藏「${item.name}」`);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

watch(
  () => props.active,
  (active) => {
    if (active) {
      void load();
    }
  },
);

watch(
  () => uiStore.favoritesVersion,
  () => {
    if (props.active) {
      void load();
    }
  },
);

onMounted(() => {
  if (props.active) {
    void load();
  }
});

defineExpose({ reload: load });
</script>

<template>
  <div class="panel">
    <div class="panel__head">
      <span class="panel__count">{{ favorites.length }} 条收藏</span>
    </div>

    <p v-if="loading" class="panel__hint">加载中…</p>
    <p v-else-if="errorMessage" class="panel__error">{{ errorMessage }}</p>

    <AppEmpty
      v-else-if="filtered.length === 0"
      title="还没有收藏"
      description="在 SQL 编辑器点击「收藏」即可把常用语句保存到这里。"
      compact
    />

    <ul v-else class="list">
      <li v-for="item in filtered" :key="item.id" class="item">
        <button type="button" class="item__main" :title="item.sqlText" @click="open(item)">
          <span class="item__name">
            <Star :size="11" class="item__star" />
            {{ item.name }}
          </span>
          <span class="item__sql">{{ compactSql(item.sqlText) }}</span>
          <span class="item__meta">
            <span>{{ formatRelativeTime(item.updatedAt) }}</span>
            <span v-if="item.databaseName">{{ item.databaseName }}</span>
          </span>
        </button>
        <button
          type="button"
          class="item__remove"
          title="删除收藏"
          @click.stop="remove(item)"
        >
          <Trash2 :size="12" />
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.panel__head {
  display: flex;
  flex: none;
  align-items: center;
  height: 26px;
  padding: 0 var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  border-bottom: 1px solid var(--divider-color);
}

.panel__hint,
.panel__error {
  padding: var(--space-3);
  font-size: var(--font-size-xs);
}

.panel__hint {
  color: var(--text-tertiary);
}

.panel__error {
  color: var(--danger-color);
}

.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.item {
  position: relative;
  display: flex;
  align-items: stretch;
}

.item + .item {
  border-top: 1px solid var(--divider-color);
}

.item__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  padding: 6px var(--space-3);
  text-align: left;
}

.item__main:hover {
  background-color: var(--surface-hover);
}

.item__name {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow: hidden;
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item__star {
  flex: none;
  color: var(--warning-color);
}

.item__sql {
  overflow: hidden;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item__meta {
  display: flex;
  gap: var(--space-2);
  color: var(--text-tertiary);
  font-size: 10px;
}

.item__remove {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 26px;
  color: transparent;
  border-radius: var(--radius-xs);
}

.item:hover .item__remove {
  color: var(--text-tertiary);
}

.item__remove:hover {
  color: var(--danger-color);
  background-color: var(--surface-hover);
}
</style>
