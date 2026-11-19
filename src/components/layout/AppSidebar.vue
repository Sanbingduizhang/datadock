<script setup lang="ts">
import { Download, PanelLeftClose, Plus, RefreshCw, Search, Upload } from 'lucide-vue-next';
import { computed, onMounted, ref } from 'vue';

import { open, save } from '@tauri-apps/plugin-dialog';

import AppEmpty from '@/components/common/AppEmpty.vue';
import AppIconButton from '@/components/common/AppIconButton.vue';
import AppInput from '@/components/common/AppInput.vue';
import ConnectionTree from '@/components/database/ConnectionTree.vue';
import FavoriteListPanel from '@/components/sidebar/FavoriteListPanel.vue';
import HistoryListPanel from '@/components/sidebar/HistoryListPanel.vue';

import { useToast } from '@/composables/useToast';
import { useConnectionStore } from '@/stores/connection';
import { useExplorerStore } from '@/stores/explorer';
import { useUiStore } from '@/stores/ui';
import { readSqlFile, writeSqlFile } from '@/api/file';

import type { ConnectionInfo } from '@/types/connection';
import type { WorkspaceTabContext } from '@/types/workspace';

type SidebarPanel = 'resources' | 'history' | 'favorites';

/**
 * 左侧导航：资源树 / 查询历史 / SQL 收藏三个面板共用同一列宽度。
 */
const emit = defineEmits<{
  'create-connection': [];
  'edit-connection': [connection: ConnectionInfo];
  'delete-connection': [connection: ConnectionInfo];
  'open-table': [context: WorkspaceTabContext];
  'open-structure': [context: WorkspaceTabContext];
  'show-database-ddl': [context: WorkspaceTabContext];
  'new-query': [context: WorkspaceTabContext];
  'open-redis-key': [context: WorkspaceTabContext];
  'open-redis-db': [context: WorkspaceTabContext];
  'close-database': [context: WorkspaceTabContext];
  'open-sql': [payload: { sql: string; connectionId?: string; databaseName?: string }];
}>();

const TABS: { key: SidebarPanel; label: string }[] = [
  { key: 'resources', label: '资源' },
  { key: 'history', label: '历史' },
  { key: 'favorites', label: '收藏' },
];

const uiStore = useUiStore();
const connectionStore = useConnectionStore();
const explorerStore = useExplorerStore();
const toast = useToast();

const panel = ref<SidebarPanel>('resources');
const keyword = ref('');
const refreshing = ref(false);

const hasConnections = computed(() => connectionStore.connections.length > 0);
const connectedCount = computed(() => connectionStore.connectedCount);

const panelTitle = computed(() => {
  switch (panel.value) {
    case 'history':
      return '查询历史';
    case 'favorites':
      return 'SQL 收藏';
    default:
      return '数据源';
  }
});

const searchPlaceholder = computed(() => {
  switch (panel.value) {
    case 'history':
      return '搜索历史 SQL';
    case 'favorites':
      return '搜索收藏名称或 SQL';
    default:
      return '搜索连接、数据库或表';
  }
});

function switchPanel(next: SidebarPanel): void {
  if (panel.value === next) {
    return;
  }
  panel.value = next;
  keyword.value = '';
}

async function refreshAll(): Promise<void> {
  refreshing.value = true;
  try {
    await connectionStore.load();
    explorerStore.clear();
    const connected = connectionStore.connections.filter((item) => item.connected);
    await Promise.all(connected.map((item) => explorerStore.loadDatabases(item.id, true)));
    toast.success('连接列表已刷新');
  } finally {
    refreshing.value = false;
  }
}

/** 导出连接配置到 JSON 文件（不含密码） */
async function exportConnections(): Promise<void> {
  const path = await save({
    title: '导出连接配置',
    defaultPath: 'datadock-connections.json',
    filters: [{ name: 'DataDock 连接文件', extensions: ['json'] }],
  });
  if (!path) {
    return;
  }

  const file = await connectionStore.exportAll();
  if (!file) {
    return;
  }

  const written = await writeSqlFile(path, JSON.stringify(file, null, 2));
  toast.success(`连接配置已导出到 ${written.path}`);
}

/** 从 JSON 文件导入连接配置（不含密码，导入后需编辑补密码） */
async function importConnections(): Promise<void> {
  const path = await open({
    title: '导入连接配置',
    multiple: false,
    filters: [{ name: 'DataDock 连接文件', extensions: ['json'] }],
  });
  if (!path || Array.isArray(path)) {
    return;
  }

  const file = await readSqlFile(path);
  await connectionStore.importAll(file.content);
}

onMounted(() => {
  if (!connectionStore.loaded && !connectionStore.loading) {
    void connectionStore.load();
  }
});
</script>

<template>
  <aside class="sidebar" :style="{ width: `${uiStore.sidebarWidth}px` }">
    <header class="sidebar__head">
      <span class="sidebar__title">{{ panelTitle }}</span>
      <span v-if="panel === 'resources' && hasConnections" class="sidebar__badge">
        {{ connectedCount }}/{{ connectionStore.connections.length }}
      </span>
      <span class="sidebar__head-spacer"></span>
      <template v-if="panel === 'resources'">
        <AppIconButton label="新建连接" @click="emit('create-connection')">
          <Plus :size="16" />
        </AppIconButton>
        <AppIconButton
          :label="refreshing ? '刷新中…' : '刷新连接与对象'"
          :disabled="refreshing"
          @click="refreshAll"
        >
          <RefreshCw :size="15" />
        </AppIconButton>
        <AppIconButton label="导入连接配置" @click="importConnections">
          <Upload :size="15" />
        </AppIconButton>
        <AppIconButton label="导出连接配置" :disabled="!hasConnections" @click="exportConnections">
          <Download :size="15" />
        </AppIconButton>
      </template>
      <AppIconButton label="收起导航栏" @click="uiStore.toggleSidebar()">
        <PanelLeftClose :size="16" />
      </AppIconButton>
    </header>

    <nav class="sidebar__tabs" role="tablist">
      <button
        v-for="tab in TABS"
        :key="tab.key"
        type="button"
        class="sidebar__tab"
        :class="{ 'is-active': panel === tab.key }"
        role="tab"
        :aria-selected="panel === tab.key"
        @click="switchPanel(tab.key)"
      >
        {{ tab.label }}
      </button>
    </nav>

    <div class="sidebar__search">
      <AppInput v-model="keyword" size="sm" :placeholder="searchPlaceholder">
        <template #prefix>
          <Search :size="13" />
        </template>
      </AppInput>
    </div>

    <div class="sidebar__body" :class="{ 'is-flat': panel !== 'resources' }">
      <template v-if="panel === 'resources'">
        <p v-if="connectionStore.loading && !hasConnections" class="sidebar__loading">
          正在读取连接…
        </p>

        <AppEmpty
          v-else-if="!hasConnections"
          title="还没有数据库连接"
          description="创建一个 MySQL 连接后，即可浏览数据库、表和视图。"
        >
          <template #actions>
            <button class="sidebar__cta" type="button" @click="emit('create-connection')">
              <Plus :size="14" />
              新建连接
            </button>
          </template>
        </AppEmpty>

        <ConnectionTree
          v-else
          :connections="connectionStore.connections"
          :keyword="keyword"
          @new-connection="emit('create-connection')"
          @edit="emit('edit-connection', $event)"
          @delete="emit('delete-connection', $event)"
          @open-table="emit('open-table', $event)"
          @open-structure="emit('open-structure', $event)"
          @show-database-ddl="emit('show-database-ddl', $event)"
          @new-query="emit('new-query', $event)"
          @open-redis-key="emit('open-redis-key', $event)"
          @open-redis-db="emit('open-redis-db', $event)"
          @close-database="emit('close-database', $event)"
        />
      </template>

      <HistoryListPanel
        v-else-if="panel === 'history'"
        :active="panel === 'history'"
        :keyword="keyword"
        @open-sql="emit('open-sql', $event)"
      />

      <FavoriteListPanel
        v-else
        :active="panel === 'favorites'"
        :keyword="keyword"
        @open-sql="emit('open-sql', $event)"
      />
    </div>

    <footer class="sidebar__foot">
      <template v-if="panel === 'resources'">
        数据源 · {{ connectedCount > 0 ? `${connectedCount} 个连接已建立` : '未建立连接' }}
      </template>
      <template v-else-if="panel === 'history'">最近 200 条执行记录</template>
      <template v-else>点击收藏项可回填到新查询</template>
    </footer>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex: none;
  flex-direction: column;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  background-color: var(--surface-bg);
}

.sidebar__head {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 34px;
  padding: 0 var(--space-1) 0 var(--space-3);
  border-bottom: 1px solid var(--divider-color);
}

.sidebar__title {
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.sidebar__badge {
  padding: 0 5px;
  color: var(--text-tertiary);
  font-size: 10px;
  line-height: 15px;
  border: 1px solid var(--border-color);
  border-radius: 999px;
}

.sidebar__head-spacer {
  flex: 1;
}

.sidebar__tabs {
  display: flex;
  flex: none;
  gap: 2px;
  height: 28px;
  padding: 0 var(--space-2);
  border-bottom: 1px solid var(--divider-color);
}

.sidebar__tab {
  flex: 1;
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  border-bottom: 2px solid transparent;
}

.sidebar__tab:hover {
  color: var(--text-secondary);
}

.sidebar__tab.is-active {
  color: var(--text-primary);
  border-bottom-color: var(--primary-color);
}

.sidebar__search {
  padding: var(--space-2) var(--space-2) var(--space-1);
}

.sidebar__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}

.sidebar__body.is-flat {
  overflow: hidden;
}

.sidebar__loading {
  padding: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  text-align: center;
}

.sidebar__cta {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  color: var(--text-inverse);
  font-size: var(--font-size-xs);
  background-color: var(--primary-color);
  border-radius: var(--radius-sm);
}

.sidebar__cta:hover {
  background-color: var(--primary-hover);
}

.sidebar__foot {
  flex: none;
  padding: 6px var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  border-top: 1px solid var(--divider-color);
}
</style>
