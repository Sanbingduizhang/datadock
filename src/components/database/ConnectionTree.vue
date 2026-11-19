<script setup lang="ts">
import {
  ChevronRight,
  Database,
  Eye,
  FolderOpen,
  Layers,
  LoaderCircle,
  Server,
  Table2,
} from 'lucide-vue-next';
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue';

import AppMenu from '@/components/common/AppMenu.vue';
import AppModal from '@/components/common/AppModal.vue';

import { useToast } from '@/composables/useToast';
import { useConnectionStore } from '@/stores/connection';
import { useExplorerStore } from '@/stores/explorer';

import type { ConnectionInfo } from '@/types/connection';
import type { DatabaseInfo, TableInfo } from '@/types/database';
import type { MenuItem } from '@/types/ui';
import type { WorkspaceTabContext } from '@/types/workspace';

/**
 * 数据库资源树：连接 → 数据库（PG 为 schema）→ 表/视图。
 * 展开节点时按需向后端加载，不一次性拉取全部对象。
 */
const props = defineProps<{
  connections: ConnectionInfo[];
  keyword: string;
}>();

const emit = defineEmits<{
  'open-table': [context: WorkspaceTabContext];
  'open-structure': [context: WorkspaceTabContext];
  'show-database-ddl': [context: WorkspaceTabContext];
  'new-query': [context: WorkspaceTabContext];
  'open-redis-db': [context: WorkspaceTabContext];
  'close-database': [context: WorkspaceTabContext];
  edit: [connection: ConnectionInfo];
  delete: [connection: ConnectionInfo];
  'new-connection': [];
}>();

const connectionStore = useConnectionStore();
const explorerStore = useExplorerStore();
const toast = useToast();

const expandedConnections = reactive(new Set<string>());
const expandedDatabases = reactive(new Set<string>());
const selectedKey = ref<string | null>(null);

const filtering = computed(() => props.keyword.trim().length > 0);
const lowerKeyword = computed(() => props.keyword.trim().toLowerCase());

function matches(text: string): boolean {
  return lowerKeyword.value.length === 0 || text.toLowerCase().includes(lowerKeyword.value);
}

/** PostgreSQL 的第二层节点是 schema，MySQL 是物理数据库 */
function isPostgres(connection: ConnectionInfo): boolean {
  return connection.sourceType === 'postgres';
}

function nodeLabel(connection: ConnectionInfo): string {
  return isPostgres(connection) ? 'schema' : '数据库';
}

/** Redis 连接下数据库层不显示表/视图，而是显示键 */
function isRedisConn(connection: ConnectionInfo): boolean {
  return connection.sourceType === 'redis';
}

/** 连接类型显示文案 */
function engineLabel(sourceType: ConnectionInfo['sourceType']): string {
  switch (sourceType) {
    case 'redis': return 'Redis';
    case 'postgres': return 'PostgreSQL';
    default: return 'MySQL';
  }
}

interface VisibleDatabase {
  database: DatabaseInfo;
  tables: TableInfo[];
  views: TableInfo[];
  state: ReturnType<typeof explorerStore.tablesState>;
}

interface VisibleConnection {
  connection: ConnectionInfo;
  databases: VisibleDatabase[];
  expanded: boolean;
  databasesState: ReturnType<typeof explorerStore.databasesState>;
  isRedis: boolean;
}

const visibleConnections = computed<VisibleConnection[]>(() =>
  props.connections
    .filter(
      (connection) => matches(connection.name) || matches(connection.host),
    )
    .map((connection) => {
      const databasesState = explorerStore.databasesState(connection.id);
      const expanded = filtering.value || expandedConnections.has(connection.id);
      const isRedis = isRedisConn(connection);

      const databases: VisibleDatabase[] = databasesState.items
        .filter((database) => matches(database.name) || filtering.value)
        .map((database) => {
          const state = explorerStore.tablesState(connection.id, database.name);
          const tables = state.items.filter((item) => item.type === 'table');
          const views = state.items.filter((item) => item.type === 'view');
          return { database, tables, views, state };
        })
        .filter((entry) => {
          if (!filtering.value) {
            return true;
          }
          return (
            matches(entry.database.name) ||
            entry.tables.some((table) => matches(table.name)) ||
            entry.views.some((view) => matches(view.name))
          );
        });

      return { connection, databases, expanded, databasesState, isRedis };
    })
    .filter((entry) => {
      if (!filtering.value || matches(entry.connection.name) || matches(entry.connection.host)) {
        return true;
      }
      return entry.databases.length > 0;
    }),
);

/* ---------- 展开 / 加载 ---------- */

function isDatabaseExpanded(connectionId: string, database: string): boolean {
  return filtering.value || expandedDatabases.has(`${connectionId}/${database}`);
}

/** 从 Redis 库名（db0 / db3）解析出库索引，失败返回 0 */
function redisDbIndex(database: string): number {
  const matched = database.match(/^db(\d+)$/i);
  return matched ? Number(matched[1]) : 0;
}

async function toggleConnection(connection: ConnectionInfo): Promise<void> {
  if (!connection.connected) {
    await ensureConnected(connection);
    return;
  }

  if (expandedConnections.has(connection.id)) {
    expandedConnections.delete(connection.id);
    return;
  }

  expandedConnections.add(connection.id);
  await explorerStore.loadDatabases(connection.id);
}

async function ensureConnected(connection: ConnectionInfo): Promise<boolean> {
  const ok = await connectionStore.connect(connection.id);
  if (!ok) {
    return false;
  }

  expandedConnections.add(connection.id);
  await explorerStore.loadDatabases(connection.id, true);
  return true;
}

async function toggleDatabase(connection: ConnectionInfo, database: string): Promise<void> {
  const key = `${connection.id}/${database}`;
  // Redis：双击库节点直接在右侧主区域打开浏览器，不再在树内展开键
  if (isRedisConn(connection)) {
    emit('open-redis-db', {
      connectionId: connection.id,
      connectionName: connection.name,
      databaseName: database,
      redisDbIndex: redisDbIndex(database),
    });
    return;
  }

  if (expandedDatabases.has(key)) {
    expandedDatabases.delete(key);
    return;
  }

  expandedDatabases.add(key);
  await explorerStore.loadTables(connection.id, database);
}

/** 库节点单击：Redis 直接打开右侧浏览（用户预期点一下就看到键），关系库仅选中 */
function handleDatabaseClick(connection: ConnectionInfo, database: string): void {
  selectKey(`${connection.id}/${database}`);
  if (isRedisConn(connection)) {
    emit('open-redis-db', {
      connectionId: connection.id,
      connectionName: connection.name,
      databaseName: database,
      redisDbIndex: redisDbIndex(database),
    });
  }
}

async function refreshConnection(connection: ConnectionInfo): Promise<void> {
  if (!connection.connected) {
    toast.info('请先连接数据库');
    return;
  }

  explorerStore.resetConnection(connection.id);
  await explorerStore.loadDatabases(connection.id, true);
  expandedConnections.add(connection.id);
  toast.success('对象已刷新');
}

async function disconnect(connection: ConnectionInfo): Promise<void> {
  const ok = await connectionStore.disconnect(connection.id);
  if (ok) {
    explorerStore.resetConnection(connection.id);
    expandedConnections.delete(connection.id);
    expandedDatabases.forEach((key) => {
      if (key.startsWith(`${connection.id}/`)) {
        expandedDatabases.delete(key);
      }
    });
  }
}

/* ---------- 打开工作区 ---------- */

function openTable(connection: ConnectionInfo, database: string, table: TableInfo): void {
  emit('open-table', {
    connectionId: connection.id,
    connectionName: connection.name,
    databaseName: database,
    tableName: table.name,
  });
}

function openStructure(
  connection: ConnectionInfo,
  database: string,
  table: TableInfo,
): void {
  emit('open-structure', {
    connectionId: connection.id,
    connectionName: connection.name,
    databaseName: database,
    tableName: table.name,
  });
}

function newQuery(connection: ConnectionInfo, database?: string): void {
  emit('new-query', {
    connectionId: connection.id,
    connectionName: connection.name,
    databaseName: database,
  });
}

async function copyName(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(`已复制「${text}」`);
  } catch {
    toast.error('复制失败，请检查剪贴板权限');
  }
}

/* ---------- 右键菜单 ---------- */

const menuVisible = ref(false);
const menuPosition = ref({ x: 0, y: 0 });
const menuItems = ref<MenuItem[]>([]);
const menuHandler = ref<((key: string) => void) | null>(null);

const MENU_WIDTH = 200;

function openMenu(event: MouseEvent, items: MenuItem[], handler: (key: string) => void): void {
  menuItems.value = items;
  menuHandler.value = handler;
  menuPosition.value = {
    x: Math.min(event.clientX, window.innerWidth - MENU_WIDTH - 8),
    y: Math.min(event.clientY, window.innerHeight - 260),
  };
  menuVisible.value = true;
}

function closeMenu(): void {
  menuVisible.value = false;
  menuHandler.value = null;
}

function handleMenuSelect(key: string): void {
  const handler = menuHandler.value;
  closeMenu();
  handler?.(key);
}

function handleConnectionMenu(event: MouseEvent, connection: ConnectionInfo): void {
  openMenu(
    event,
    [
      { key: 'connect', label: '连接', disabled: connection.connected },
      { key: 'disconnect', label: '断开连接', disabled: !connection.connected },
      { key: 'edit', label: '编辑连接' },
      { key: 'menu-divider-1', label: '', divider: true },
      { key: 'new-query', label: '新建 SQL 查询' },
      { key: 'refresh', label: '刷新对象' },
      { key: 'connection-info', label: '连接信息' },
      { key: 'menu-divider-2', label: '', divider: true },
      { key: 'copy-name', label: '复制连接名称' },
      { key: 'delete', label: '删除连接', danger: true },
    ],
    (key) => {
      switch (key) {
        case 'connect':
          void ensureConnected(connection);
          break;
        case 'disconnect':
          void disconnect(connection);
          break;
        case 'edit':
          emit('edit', connection);
          break;
        case 'new-query':
          newQuery(connection);
          break;
        case 'refresh':
          void refreshConnection(connection);
          break;
        case 'connection-info':
          showConnectionInfo(connection);
          break;
        case 'copy-name':
          void copyName(connection.name);
          break;
        case 'delete':
          emit('delete', connection);
          break;
        default:
          break;
      }
    },
  );
}

function handleDatabaseMenu(
  event: MouseEvent,
  connection: ConnectionInfo,
  database: DatabaseInfo,
): void {
  const label = nodeLabel(connection);
  const items: MenuItem[] = [
    { key: 'new-query', label: '新建 SQL 查询' },
    ...(isPostgres(connection) ? [] : [{ key: 'show-ddl', label: '查看建库语句' }]),
    { key: 'refresh', label: '刷新' },
    { key: 'menu-divider-db', label: '', divider: true },
    { key: 'copy-name', label: `复制${label}名称` },
    { key: 'close-db', label: '关闭数据库' },
  ];
  openMenu(
    event,
    items,
    (key) => {
      if (key === 'new-query') {
        newQuery(connection, database.name);
      } else if (key === 'show-ddl') {
        emit('show-database-ddl', {
          connectionId: connection.id,
          connectionName: connection.name,
          databaseName: database.name,
        });
      } else if (key === 'refresh') {
        void explorerStore.loadTables(connection.id, database.name, true);
      } else if (key === 'copy-name') {
        void copyName(database.name);
      } else if (key === 'close-db') {
        // 关闭该数据库：收起并清空该库缓存（不断开连接）
        expandedDatabases.delete(`${connection.id}/${database.name}`);
        emit('close-database', {
          connectionId: connection.id,
          connectionName: connection.name,
          databaseName: database.name,
          redisDbIndex: isRedisConn(connection) ? redisDbIndex(database.name) : undefined,
        });
      }
    },
  );
}

function handleTableMenu(
  event: MouseEvent,
  connection: ConnectionInfo,
  database: string,
  table: TableInfo,
): void {
  openMenu(
    event,
    [
      { key: 'open-data', label: '打开表数据' },
      { key: 'open-structure', label: '查看 / 设计表结构' },
      { key: 'menu-divider-1', label: '', divider: true },
      { key: 'copy-name', label: '复制表名' },
    ],
    (key) => {
      if (key === 'open-data') {
        openTable(connection, database, table);
      } else if (key === 'open-structure') {
        openStructure(connection, database, table);
      } else if (key === 'copy-name') {
        void copyName(table.name);
      }
    },
  );
}

function handleDocumentPointerDown(event: PointerEvent): void {
  const target = event.target as HTMLElement;
  if (!target.closest('.tree-context-menu')) {
    closeMenu();
  }
}

function handleDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    closeMenu();
  }
}

watch(menuVisible, (visible) => {
  if (visible) {
    document.addEventListener('pointerdown', handleDocumentPointerDown);
    document.addEventListener('keydown', handleDocumentKeydown);
  } else {
    document.removeEventListener('pointerdown', handleDocumentPointerDown);
    document.removeEventListener('keydown', handleDocumentKeydown);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown);
  document.removeEventListener('keydown', handleDocumentKeydown);
});

/* ---------- 选中 ---------- */

function selectKey(key: string): void {
  selectedKey.value = key;
}

function isSelected(key: string): boolean {
  return selectedKey.value === key;
}

function tableIcon(table: TableInfo) {
  return table.type === 'view' ? Eye : Table2;
}

function connectionStatusClass(connection: ConnectionInfo): string {
  return connection.connected ? 'is-on' : 'is-off';
}

/* ---------- 连接信息弹窗（打开连接后展示版本等信息） ---------- */

const infoVisible = ref(false);
const infoConnection = ref<ConnectionInfo | null>(null);

function showConnectionInfo(connection: ConnectionInfo): void {
  infoConnection.value = connection;
  infoVisible.value = true;
}
</script>

<template>
  <div class="tree" role="tree">
    <div v-if="visibleConnections.length === 0" class="tree__empty">
      <p>没有匹配的连接</p>
      <button type="button" class="tree__empty-action" @click="emit('new-connection')">
        新建连接
      </button>
    </div>

    <div v-for="entry in visibleConnections" :key="entry.connection.id" class="node">
      <!-- 连接 -->
      <div
        class="row row--connection"
        :class="{ 'is-selected': isSelected(entry.connection.id) }"
        :title="`${entry.connection.name} · ${entry.connection.username}@${entry.connection.host}:${entry.connection.port}`"
        @click="selectKey(entry.connection.id)"
        @dblclick="toggleConnection(entry.connection)"
        @contextmenu.prevent="handleConnectionMenu($event, entry.connection)"
      >
        <button
          type="button"
          class="chevron"
          :class="{ 'is-expanded': entry.expanded }"
          :aria-label="entry.expanded ? '收起' : '展开'"
          @click.stop="toggleConnection(entry.connection)"
        >
          <ChevronRight :size="12" />
        </button>

        <Server :size="14" class="icon icon--connection" />
        <span class="label">{{ entry.connection.name }}</span>

        <span
          v-if="connectionStore.busyConnectionId === entry.connection.id"
          class="busy"
          title="处理中"
        >
          <LoaderCircle :size="12" class="spin" />
        </span>
        <span v-else class="status" :class="connectionStatusClass(entry.connection)"></span>
      </div>

      <!-- 连接下：MySQL 为数据库，PostgreSQL 为 schema -->
      <div v-if="entry.expanded" class="children">
        <p v-if="entry.databasesState.status === 'loading'" class="hint-row">
          <LoaderCircle :size="12" class="spin" /> 正在加载{{ nodeLabel(entry.connection) }}…
        </p>

        <p v-else-if="entry.databasesState.status === 'error'" class="error-row">
          {{ entry.databasesState.error ?? '加载失败' }}
          <button
            type="button"
            class="retry"
            @click="explorerStore.loadDatabases(entry.connection.id, true)"
          >
            重试
          </button>
        </p>

        <p
          v-else-if="entry.databases.length === 0 && entry.databasesState.status === 'ready'"
          class="hint-row"
        >
          没有可访问的{{ nodeLabel(entry.connection) }}
        </p>

        <div v-for="dbEntry in entry.databases" :key="dbEntry.database.name" class="node">
          <div
            class="row row--database"
            :class="{ 'is-selected': isSelected(`${entry.connection.id}/${dbEntry.database.name}`) }"
            @click="handleDatabaseClick(entry.connection, dbEntry.database.name)"
            @dblclick="toggleDatabase(entry.connection, dbEntry.database.name)"
            @contextmenu.prevent="handleDatabaseMenu($event, entry.connection, dbEntry.database)"
          >
            <button
              type="button"
              class="chevron"
              :class="{ 'is-expanded': isDatabaseExpanded(entry.connection.id, dbEntry.database.name) }"
              :aria-label="'展开' + nodeLabel(entry.connection)"
              @click.stop="toggleDatabase(entry.connection, dbEntry.database.name)"
            >
              <ChevronRight :size="12" />
            </button>

            <component
              :is="isPostgres(entry.connection) ? Layers : Database"
              :size="14"
              class="icon"
            />
            <span class="label">{{ dbEntry.database.name }}</span>
          </div>

          <!-- Redis 的键在右侧主区域浏览器中展示（双击库节点打开），树内不再展开键列表 -->
          <div
            v-if="!entry.isRedis && isDatabaseExpanded(entry.connection.id, dbEntry.database.name)"
            class="children"
          >
            <!-- 关系库：表 / 视图 -->
            <p v-if="dbEntry.state.status === 'loading'" class="hint-row">
              <LoaderCircle :size="12" class="spin" /> 正在加载对象…
            </p>

            <p v-else-if="dbEntry.state.status === 'error'" class="error-row">
              {{ dbEntry.state.error ?? '加载失败' }}
              <button
                type="button"
                class="retry"
                @click="explorerStore.loadTables(entry.connection.id, dbEntry.database.name, true)"
              >
                重试
              </button>
            </p>

            <template v-else>
              <div v-if="dbEntry.tables.length === 0 && dbEntry.views.length === 0" class="hint-row">
                该{{ nodeLabel(entry.connection) }}还没有表
              </div>

              <div v-if="dbEntry.tables.length > 0" class="group">
                <FolderOpen :size="12" class="icon icon--group" />
                <span>表 · {{ dbEntry.tables.length }}</span>
              </div>
              <div
                v-for="table in dbEntry.tables"
                :key="table.name"
                class="row row--object"
                :class="{ 'is-selected': isSelected(`${entry.connection.id}/${dbEntry.database.name}/${table.name}`) }"
                :title="table.comment ? `${table.name} · ${table.comment}` : table.name"
                @click="selectKey(`${entry.connection.id}/${dbEntry.database.name}/${table.name}`)"
                @dblclick="openTable(entry.connection, dbEntry.database.name, table)"
                @contextmenu.prevent="handleTableMenu($event, entry.connection, dbEntry.database.name, table)"
              >
                <component :is="tableIcon(table)" :size="13" class="icon icon--object" />
                <span class="label">{{ table.name }}</span>
                <span v-if="table.comment" class="comment">{{ table.comment }}</span>
              </div>

              <div v-if="dbEntry.views.length > 0" class="group">
                <FolderOpen :size="12" class="icon icon--group" />
                <span>视图 · {{ dbEntry.views.length }}</span>
              </div>
              <div
                v-for="view in dbEntry.views"
                :key="view.name"
                class="row row--object"
                @click="selectKey(`${entry.connection.id}/${dbEntry.database.name}/${view.name}`)"
                @dblclick="openTable(entry.connection, dbEntry.database.name, view)"
                @contextmenu.prevent="handleTableMenu($event, entry.connection, dbEntry.database.name, view)"
              >
                <component :is="tableIcon(view)" :size="13" class="icon icon--object" />
                <span class="label">{{ view.name }}</span>
              </div>
            </template>
          </div>
        </div>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="menuVisible"
        class="tree-context-menu"
        :style="{ left: `${menuPosition.x}px`, top: `${menuPosition.y}px` }"
      >
        <AppMenu :items="menuItems" :min-width="MENU_WIDTH - 8" @select="handleMenuSelect" />
      </div>
    </Teleport>

    <!-- 连接信息：展示主机、端口、版本等（类似 Navicat 打开连接后的信息面板） -->
    <AppModal v-model="infoVisible" title="连接信息" :width="480" hide-footer>
      <template v-if="infoConnection">
        <div class="conn-info">
          <div class="conn-info__head">
            <span class="conn-info__name">
              <Server :size="16" />
              {{ infoConnection.name }}
            </span>
            <span
              class="conn-info__status"
              :class="infoConnection.connected ? 'is-on' : 'is-off'"
            >
              {{ infoConnection.connected ? '已连接' : '未连接' }}
            </span>
          </div>

          <dl class="conn-info__rows">
            <div class="conn-info__row">
              <dt>类型</dt>
              <dd>{{ engineLabel(infoConnection.sourceType) }}</dd>
            </div>
            <div class="conn-info__row">
              <dt>主机</dt>
              <dd>{{ infoConnection.host }}</dd>
            </div>
            <div class="conn-info__row">
              <dt>端口</dt>
              <dd>{{ infoConnection.port }}</dd>
            </div>
            <div class="conn-info__row">
              <dt>用户名</dt>
              <dd>{{ infoConnection.username || '—' }}</dd>
            </div>
            <div class="conn-info__row">
              <dt>默认数据库</dt>
              <dd>{{ infoConnection.defaultDatabase || '—' }}</dd>
            </div>
            <div v-if="infoConnection.serverVersion" class="conn-info__row">
              <dt>服务器版本</dt>
              <dd class="is-mono">{{ infoConnection.serverVersion }}</dd>
            </div>
          </dl>
        </div>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.tree {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: var(--space-1) var(--space-1) var(--space-4);
}

.tree__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  text-align: center;
}

.tree__empty-action {
  color: var(--primary-color);
  font-size: var(--font-size-xs);
}

.row {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 25px;
  padding-right: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  border-radius: var(--radius-sm);
  cursor: default;
  user-select: none;
}

.row:hover {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}

.row.is-selected {
  color: var(--text-primary);
  background-color: var(--surface-selected);
}

.row--connection {
  padding-left: 4px;
  font-weight: 500;
}

.row--database {
  padding-left: 18px;
}

.row--object {
  padding-left: 46px;
}

.chevron {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  color: var(--text-tertiary);
  transition: transform var(--transition-fast);
}

.chevron.is-expanded {
  transform: rotate(90deg);
}

.icon {
  flex: none;
  color: var(--text-tertiary);
}

.icon--connection {
  color: var(--primary-color);
}

.icon--group {
  color: var(--text-tertiary);
}

.icon--object {
  color: var(--text-secondary);
}

.label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.comment {
  flex: none;
  max-width: 42%;
  overflow: hidden;
  color: var(--text-tertiary);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status {
  flex: none;
  width: 6px;
  height: 6px;
  margin-left: auto;
  border-radius: 50%;
}

.status.is-on {
  background-color: var(--success-color);
}

.status.is-off {
  background-color: var(--border-strong);
}

.busy {
  display: inline-flex;
  margin-left: auto;
  color: var(--primary-color);
}

.group {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding-left: 34px;
  color: var(--text-tertiary);
  font-size: 11px;
  user-select: none;
}

.hint-row,
.error-row {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 8px 4px 34px;
  color: var(--text-tertiary);
  font-size: 11px;
}

.error-row {
  color: var(--danger-color);
}

.retry {
  color: var(--primary-color);
  font-size: 11px;
}

.spin {
  animation: spin 900ms linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.tree-context-menu {
  position: fixed;
  z-index: var(--z-dropdown);
}

.conn-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.conn-info__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.conn-info__name {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  font-weight: 500;
}

.conn-info__status {
  flex: none;
  padding: 1px 8px;
  font-size: 10px;
  border-radius: var(--radius-xs);
}

.conn-info__status.is-on {
  color: var(--success-color);
  background-color: var(--success-soft);
}

.conn-info__status.is-off {
  color: var(--text-tertiary);
  background-color: var(--surface-subtle);
}

.conn-info__rows {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.conn-info__row {
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: var(--space-2);
  align-items: baseline;
  padding: 5px 0;
  border-bottom: 1px solid var(--divider-color);
}

.conn-info__row:last-child {
  border-bottom: none;
}

.conn-info__row dt {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.conn-info__row dd {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--font-size-xs);
  word-break: break-all;
}

.conn-info__row dd.is-mono {
  font-family: var(--font-mono);
}
</style>
