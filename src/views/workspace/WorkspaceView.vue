<script setup lang="ts">
import { storeToRefs } from 'pinia';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';

import { getCurrentWindow } from '@tauri-apps/api/window';
import { open, save } from '@tauri-apps/plugin-dialog';

import AppButton from '@/components/common/AppButton.vue';
import AppInput from '@/components/common/AppInput.vue';
import AppModal from '@/components/common/AppModal.vue';
import ConnectionDialog from '@/components/database/ConnectionDialog.vue';
import DumpDialog from '@/components/database/DumpDialog.vue';
import SettingsDialog from '@/components/settings/SettingsDialog.vue';
import AppMenuBar from '@/components/layout/AppMenuBar.vue';
import AppResultPanel from '@/components/layout/AppResultPanel.vue';
import AppSidebar from '@/components/layout/AppSidebar.vue';
import AppStatusBar from '@/components/layout/AppStatusBar.vue';
import SqlLogPanel from '@/components/layout/SqlLogPanel.vue';
import AppToolbar from '@/components/layout/AppToolbar.vue';
import ResizeHandle from '@/components/layout/ResizeHandle.vue';
import WorkspaceTabs from '@/components/layout/WorkspaceTabs.vue';
import QueryTab from '@/views/workspace/tabs/QueryTab.vue';
import StructureTab from '@/views/workspace/tabs/StructureTab.vue';
import TableTab from '@/views/workspace/tabs/TableTab.vue';
import WelcomeTab from '@/views/workspace/tabs/WelcomeTab.vue';
import RedisKeyTab from '@/views/workspace/tabs/RedisKeyTab.vue';
import RedisDbBrowser from '@/views/workspace/tabs/RedisDbBrowser.vue';

import { readSqlFile, writeSqlFile } from '@/api/file';
import { getDatabaseDdl } from '@/api/schema';
import { useToast } from '@/composables/useToast';
import { useWorkspacePersistence } from '@/composables/useWorkspacePersistence';
import { useAppStore } from '@/stores/app';
import { useConnectionStore } from '@/stores/connection';
import { useExplorerStore } from '@/stores/explorer';
import { useSqlLogStore } from '@/stores/sql-log';
import { useUiStore } from '@/stores/ui';
import { useTransactionStore } from '@/stores/transaction';
import { useWorkspaceStore } from '@/stores/workspace';
import { DEFAULT_WORKSPACE_ID, useWorkspacesStore } from '@/stores/workspaces';

import { copyText } from '@/utils/clipboard';
import { normalizeError } from '@/utils/errors';

import type { DatabaseDdl } from '@/types/database';

import type { ConnectionInfo } from '@/types/connection';
import type { WorkspaceTabContext } from '@/types/workspace';

/**
 * 主工作区：负责主界面布局组装与各区域之间的动作调度。
 * 组件本身不直接操作数据库，也不保存连接凭据。
 */
const appStore = useAppStore();
const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const txStore = useTransactionStore();
const sqlLogStore = useSqlLogStore();
const workspaces = useWorkspacesStore();
const connectionStore = useConnectionStore();
const explorerStore = useExplorerStore();
const toast = useToast();
const persistence = useWorkspacePersistence();

/* ---------- 连接对话框 ---------- */

const dialogVisible = ref(false);
const dialogMode = ref<'create' | 'edit'>('create');
const editingConnection = ref<ConnectionInfo | null>(null);

/* ---------- 数据库导入导出 ---------- */

const dumpDialogVisible = ref(false);
const dumpDialogMode = ref<'import' | 'export'>('export');

const { tabs, activeTabId, activeTab, tabCount } = storeToRefs(workspaceStore);
const { appInfo } = storeToRefs(appStore);

const workspaceRef = ref<HTMLElement | null>(null);
const queryTabRef = ref<{ execute: () => Promise<void> } | null>(null);
const aboutVisible = ref(false);
const settingsVisible = ref(false);
const databaseDdlVisible = ref(false);
const databaseDdlLoading = ref(false);
const databaseDdl = ref<DatabaseDdl | null>(null);
const workspaceModalVisible = ref(false);
const workspaceMode = ref<'create' | 'rename'>('create');
const workspaceNameInput = ref('');
const savingWorkspace = ref(false);

/* ---------- 通用确认弹窗 ---------- */

interface ConfirmPayload {
  title: string;
  message: string;
  confirmText: string;
  onConfirm: () => void;
}

const confirmVisible = ref(false);
const confirmPayload = ref<ConfirmPayload | null>(null);

function askConfirm(payload: ConfirmPayload): void {
  confirmPayload.value = payload;
  confirmVisible.value = true;
}

function handleConfirm(): void {
  confirmPayload.value?.onConfirm();
  confirmVisible.value = false;
}

function handleCancelConfirm(): void {
  confirmVisible.value = false;
}

/* ---------- 标签操作 ---------- */

const activeEngine = computed(() => {
  const connectionId = activeTab.value?.context.connectionId;
  const sourceType = connectionStore.connections.find(
    (item) => item.id === connectionId,
  )?.sourceType;
  return sourceType === 'redis'
    ? 'Redis'
    : sourceType === 'postgres'
      ? 'PostgreSQL'
      : 'MySQL';
});

const activeTransaction = computed(() =>
  activeTabId.value ? txStore.get(activeTabId.value) : null,
);

/** 关闭标签前释放事务：后端会回滚未提交的改动 */
async function releaseTransactions(tabIds: string[], silent = false): Promise<void> {
  const targets = tabIds.filter((tabId) => txStore.has(tabId));
  if (targets.length === 0) {
    return;
  }

  await Promise.allSettled(
    targets.map(async (tabId) => {
      try {
        await txStore.rollback(tabId);
      } catch {
        txStore.forget(tabId);
      }
    }),
  );

  if (!silent) {
    toast.info('未提交的事务已回滚');
  }
}

function requestCloseTab(tabId: string): void {
  const tab = tabs.value.find((item) => item.id === tabId);
  if (!tab || !tab.closable) {
    return;
  }

  const hasTransaction = txStore.has(tabId);

  if (tab.dirty || hasTransaction) {
    if (hasTransaction) {
      askConfirm({
        title: '关闭有未提交事务的标签',
        message: `「${tab.title}」有未提交的事务，关闭将回滚${
          tab.dirty ? '，未保存的 SQL 也会丢失' : ''
        }。`,
        confirmText: '回滚并关闭',
        onConfirm: () => {
          void releaseTransactions([tabId]).finally(() => workspaceStore.closeTab(tabId));
        },
      });
      return;
    }

    askConfirm({
      title: '关闭未保存的标签',
      message: `「${tab.title}」包含未保存的 SQL，关闭后内容将丢失。`,
      confirmText: '丢弃并关闭',
      onConfirm: () => workspaceStore.closeTab(tabId),
    });
    return;
  }

  workspaceStore.closeTab(tabId);
}

function requestCloseOthers(tabId: string): void {
  const hasDirtySiblings = tabs.value.some((tab) => tab.id !== tabId && tab.closable && tab.dirty);
  if (hasDirtySiblings) {
    askConfirm({
      title: '关闭其他标签',
      message: '其他标签中存在未保存的 SQL，继续操作将丢失这些内容。',
      confirmText: '继续关闭',
      onConfirm: () => {
        const ids = tabs.value
          .filter((item) => item.id !== tabId && item.closable)
          .map((item) => item.id);
        void releaseTransactions(ids).finally(() => workspaceStore.closeOtherTabs(tabId));
      },
    });
    return;
  }
  workspaceStore.closeOtherTabs(tabId);
}

function requestCloseAll(): void {
  if (workspaceStore.hasDirtyTab) {
    askConfirm({
      title: '关闭全部标签',
      message: '存在未保存的 SQL，继续操作将丢失这些内容。',
      confirmText: '继续关闭',
      onConfirm: () => {
        const ids = tabs.value.filter((item) => item.closable).map((item) => item.id);
        void releaseTransactions(ids).finally(() => workspaceStore.closeAllTabs());
      },
    });
    return;
  }
  workspaceStore.closeAllTabs();
}

function handleTabAction(actionKey: string): void {
  const [action, tabId] = actionKey.split(':');
  if (!tabId) {
    return;
  }

  if (action === 'close-others') {
    requestCloseOthers(tabId);
  } else if (action === 'close-right') {
    workspaceStore.closeTabsToRight(tabId);
  } else if (action === 'close-all') {
    requestCloseAll();
  }
}

/* ---------- 菜单与工具栏动作 ---------- */

function openNewQuery(context: WorkspaceTabContext = {}): void {
  let resolved = context;

  // 只有一个已连接的数据源时自动绑定，减少空上下文的无效操作
  if (!resolved.connectionId) {
    const connected = connectionStore.connections.filter((item) => item.connected);
    const only = connected[0];
    if (connected.length === 1 && only) {
      resolved = {
        connectionId: only.id,
        connectionName: only.name,
        ...(only.defaultDatabase ? { databaseName: only.defaultDatabase } : {}),
      };
    }
  }

  workspaceStore.openQueryTab(resolved);
}

/** 标签 → 已保存的 SQL 文件路径（仅本次运行有效） */
const sqlFilePaths = ref<Record<string, string>>({});

/** 侧栏历史/收藏：在新的查询标签中回填 SQL */
function handleOpenSavedSql(payload: {
  sql: string;
  connectionId?: string;
  databaseName?: string;
}): void {
  let context: WorkspaceTabContext = {};

  if (payload.connectionId) {
    const connection = connectionStore.connections.find(
      (item) => item.id === payload.connectionId,
    );
    context = {
      connectionId: payload.connectionId,
      connectionName: connection?.name,
      ...(payload.databaseName ? { databaseName: payload.databaseName } : {}),
    };
  }

  openNewQuery(context);
  const tab = workspaceStore.activeTab;
  if (tab) {
    workspaceStore.updateDraft(tab.id, payload.sql);
  }
}

/** 打开本地 SQL 文件到新的查询标签 */
async function handleOpenSqlFile(): Promise<void> {
  let path: string | null = null;
  try {
    path = await open({
      title: '打开 SQL 文件',
      multiple: false,
      directory: false,
      filters: [
        { name: 'SQL 文件', extensions: ['sql'] },
        { name: '所有文件', extensions: ['*'] },
      ],
    });
  } catch (raw) {
    toast.error(normalizeError(raw).message);
    return;
  }

  if (!path || typeof path !== 'string') {
    return;
  }

  try {
    const file = await readSqlFile(path);
    openNewQuery({});
    const tab = workspaceStore.activeTab;
    if (tab) {
      workspaceStore.updateDraft(tab.id, file.content);
      sqlFilePaths.value = { ...sqlFilePaths.value, [tab.id]: file.path };
      workspaceStore.setTabTitle(tab.id, file.name);
    }
    toast.success(`已打开 ${file.name}`);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

/** 保存当前查询标签的 SQL 到文件 */
async function handleSaveSqlFile(): Promise<void> {
  const tab = activeTab.value;
  if (!tab || tab.type !== 'query') {
    toast.warning('请先切换到 SQL 查询标签');
    return;
  }

  let path: string | null = sqlFilePaths.value[tab.id] ?? null;

  if (!path) {
    try {
      path = await save({
        title: '保存 SQL 文件',
        defaultPath: `${tab.title || 'query'}.sql`,
        filters: [{ name: 'SQL 文件', extensions: ['sql'] }],
      });
    } catch (raw) {
      toast.error(normalizeError(raw).message);
      return;
    }
  }

  if (!path) {
    return;
  }

  try {
    const result = await writeSqlFile(path, tab.draft ?? '');
    sqlFilePaths.value = { ...sqlFilePaths.value, [tab.id]: result.path };
    workspaceStore.setTabTitle(tab.id, result.name);
    workspaceStore.markClean(tab.id);
    toast.success(`已保存 ${result.name}`);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

/** 侧栏节点可能缺少表相关上下文，打开标签前统一校验 */
function handleOpenTable(context: WorkspaceTabContext): void {
  if (!context.databaseName || !context.tableName) {
    toast.warning('请选择具体的表节点');
    return;
  }
  workspaceStore.openTableTab({
    ...context,
    databaseName: context.databaseName,
    tableName: context.tableName,
  });
}

function handleOpenStructure(context: WorkspaceTabContext): void {
  if (!context.databaseName || !context.tableName) {
    toast.warning('请选择具体的表节点');
    return;
  }
  workspaceStore.openStructureTab({
    ...context,
    databaseName: context.databaseName,
    tableName: context.tableName,
  });
}

function handleOpenRedisKey(context: WorkspaceTabContext): void {
  if (!context.connectionId || !context.keyName) {
    toast.warning('请选择具体的 Redis 键节点');
    return;
  }
  workspaceStore.openRedisKeyTab({
    ...context,
    keyName: context.keyName,
  });
}

/** 打开 Redis 库浏览标签（右侧主区域），双击库节点触发 */
function handleOpenRedisDb(context: WorkspaceTabContext): void {
  if (!context.connectionId || !context.databaseName) {
    toast.warning('请选择具体的 Redis 库节点');
    return;
  }
  workspaceStore.openRedisDbTab({
    ...context,
    databaseName: context.databaseName,
  });
}

/** 关闭资源树中已打开的数据库（收起并清空该库缓存），不断开连接 */
function handleCloseDatabase(context: WorkspaceTabContext): void {
  if (!context.connectionId || !context.databaseName) {
    toast.warning('请选择具体的数据库节点');
    return;
  }

  const database = context.databaseName;
  explorerStore.closeDatabase(context.connectionId, database);

  // 同时关闭该库下已打开的表/结构/Redis 浏览标签
  const closable = workspaceStore.tabs.filter(
    (tab) =>
      tab.context.connectionId === context.connectionId &&
      (tab.context.databaseName === database || tab.context.redisDbIndex !== undefined) &&
      (tab.type === 'table' || tab.type === 'structure' || tab.type === 'redis-db'),
  );
  for (const tab of closable) {
    workspaceStore.closeTab(tab.id);
  }

  toast.info(`已关闭数据库 ${database}`);
}

/** 查看建库语句（资源树右键数据库节点） */
async function handleShowDatabaseDdl(context: WorkspaceTabContext): Promise<void> {
  if (!context.connectionId || !context.databaseName) {
    toast.warning('请选择具体的数据库节点');
    return;
  }

  databaseDdl.value = null;
  databaseDdlVisible.value = true;
  databaseDdlLoading.value = true;

  try {
    databaseDdl.value = await getDatabaseDdl(context.connectionId, context.databaseName);
  } catch (raw) {
    databaseDdlVisible.value = false;
    toast.error(normalizeError(raw).message);
  } finally {
    databaseDdlLoading.value = false;
  }
}

async function copyDatabaseDdl(): Promise<void> {
  const ddl = databaseDdl.value?.ddl;
  if (!ddl) {
    return;
  }
  if (await copyText(ddl)) {
    toast.success('建库语句已复制');
    return;
  }
  toast.error('复制失败，请手动选择文本复制');
}

function openCreateDialog(): void {
  dialogMode.value = 'create';
  editingConnection.value = null;
  dialogVisible.value = true;
}

function openEditDialog(connection: ConnectionInfo): void {
  dialogMode.value = 'edit';
  editingConnection.value = connection;
  dialogVisible.value = true;
}

function handleConnectionSaved(): void {
  // 连接列表由 store 自行刷新；这里只需清理已失效的资源缓存
  explorerStore.clear();
}

function handleDeleteConnection(connection: ConnectionInfo): void {
  askConfirm({
    title: '删除连接',
    message: `确定删除「${connection.name}」？仅删除本地连接配置与系统凭据，不会影响远程数据库，查询历史会保留。`,
    confirmText: '删除',
    onConfirm: () => {
      void connectionStore.remove(connection.id).then((removed) => {
        if (removed) {
          explorerStore.resetConnection(connection.id);
        }
      });
    },
  });
}

async function handleRefresh(): Promise<void> {
  await connectionStore.load();

  const connected = connectionStore.connections.filter((item) => item.connected);
  await Promise.all(connected.map((item) => explorerStore.loadDatabases(item.id, true)));

  toast.success(connected.length > 0 ? '连接与对象已刷新' : '连接列表已刷新');
}

/** 关闭主窗口（阶段 H 需要补充未保存内容的退出确认） */
async function handleExit(): Promise<void> {
  try {
    await getCurrentWindow().close();
  } catch (error) {
    toast.error(error instanceof Error ? error.message : '退出失败');
  }
}

/* ---------- 多工作区 ---------- */

async function switchWorkspace(id: string): Promise<void> {
  if (!id || id === workspaces.currentId) {
    return;
  }

  const closableIds = tabs.value.filter((tab) => tab.closable).map((tab) => tab.id);
  await releaseTransactions(closableIds, true);
  await persistence.flushNow();

  workspaces.setCurrent(id);
  workspaceStore.reset();

  const restored = await persistence.restore();
  if (!restored) {
    workspaceStore.openWelcomeTab();
  }

  await workspaces.load();
  toast.success(`已切换到「${workspaces.currentName}」`);
}

function openWorkspaceModal(mode: 'create' | 'rename'): void {
  workspaceMode.value = mode;
  workspaceNameInput.value = mode === 'rename' ? workspaces.currentName : '';
  workspaceModalVisible.value = true;
}

async function submitWorkspaceModal(): Promise<void> {
  const name = workspaceNameInput.value.trim();
  if (name.length === 0) {
    toast.warning('请输入工作区名称');
    return;
  }

  savingWorkspace.value = true;
  try {
    if (workspaceMode.value === 'rename') {
      workspaces.renameCurrent(name);
      await persistence.flushNow();
      await workspaces.load();
      toast.success('工作区已重命名');
    } else {
      const closableIds = tabs.value.filter((tab) => tab.closable).map((tab) => tab.id);
      await releaseTransactions(closableIds, true);
      await persistence.flushNow();

      const id = `ws-${Date.now().toString(36)}`;
      workspaces.setCurrent(id, name);
      workspaceStore.reset();
      workspaceStore.openWelcomeTab();
      await persistence.flushNow();
      await workspaces.load();
      toast.success(`已创建并切换到「${name}」`);
    }

    workspaceModalVisible.value = false;
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  } finally {
    savingWorkspace.value = false;
  }
}

function handleDeleteWorkspace(): void {
  if (workspaces.currentId === DEFAULT_WORKSPACE_ID) {
    toast.warning('默认工作区不能删除');
    return;
  }

  const target = workspaces.currentId;
  const targetName = workspaces.currentName;

  askConfirm({
    title: '删除工作区',
    message: `将删除工作区「${targetName}」及其保存的标签。其他工作区不受影响，此操作不可撤销。`,
    confirmText: '删除',
    onConfirm: async () => {
      try {
        await workspaces.remove(target);
        await switchWorkspace(DEFAULT_WORKSPACE_ID);
      } catch (raw) {
        toast.error(normalizeError(raw).message);
      }
    },
  });
}

async function handleBeginTransaction(): Promise<void> {
  const tab = activeTab.value;
  if (!tab || tab.type !== 'query' || !tab.context.connectionId) {
    toast.warning('请先在已连接数据库的查询标签中开启事务');
    return;
  }

  try {
    await txStore.begin(tab.id, tab.context.connectionId, tab.context.databaseName);
    toast.success('事务已开启');
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

async function handleFinishTransaction(action: 'commit' | 'rollback'): Promise<void> {
  const tabId = activeTabId.value;
  if (!tabId || !txStore.has(tabId)) {
    return;
  }

  try {
    const message =
      action === 'commit' ? await txStore.commit(tabId) : await txStore.rollback(tabId);
    toast.success(message);
  } catch (raw) {
    toast.error(normalizeError(raw).message);
  }
}

function handleMenuAction(key: string): void {
  switch (key) {
    case 'file.newConnection':
      openCreateDialog();
      break;
    case 'file.openSql':
      void handleOpenSqlFile();
      break;
    case 'file.saveSql':
      void handleSaveSqlFile();
      break;
    case 'file.exit':
      void handleExit();
      break;
    case 'tools.settings':
      settingsVisible.value = true;
      break;
    case 'database.export':
      dumpDialogMode.value = 'export';
      dumpDialogVisible.value = true;
      break;
    case 'database.import':
      dumpDialogMode.value = 'import';
      dumpDialogVisible.value = true;
      break;
    case 'view.sqlLog':
      uiStore.toggleSqlLog();
      break;
    case 'view.sidebar':
      uiStore.toggleSidebar();
      break;
    case 'view.resultPanel':
      uiStore.toggleResultPanel();
      break;
    case 'view.statusBar':
      uiStore.toggleStatusBar();
      break;
    case 'view.themeLight':
      appStore.setThemeMode('light');
      break;
    case 'view.themeDark':
      appStore.setThemeMode('dark');
      break;
    case 'view.themeGlass':
      appStore.setThemeMode('glass');
      break;
    case 'view.themeSkeuomorph':
      appStore.setThemeMode('skeuomorph');
      break;
    case 'view.themeSystem':
      appStore.setThemeMode('system');
      break;
    case 'window.closeTab':
      if (activeTab.value) {
        requestCloseTab(activeTab.value.id);
      }
      break;
    case 'window.closeOtherTabs':
      if (activeTabId.value) {
        requestCloseOthers(activeTabId.value);
      }
      break;
    case 'window.closeAllTabs':
      requestCloseAll();
      break;
    case 'help.about':
      aboutVisible.value = true;
      break;
    default:
      break;
  }
}

/* ---------- 面板拖拽 ---------- */

function handleSidebarResize(delta: number): void {
  uiStore.setSidebarWidth(uiStore.sidebarWidth + delta);
}

function handleResultResize(delta: number): void {
  const height = workspaceRef.value?.clientHeight;
  uiStore.setResultPanelHeight(uiStore.resultPanelHeight - delta, height);
}

/* ---------- 标签内容 ---------- */

/* ---------- 快捷键 ---------- */

const isMac = navigator.userAgent.includes('Mac');

function handleGlobalKeydown(event: KeyboardEvent): void {
  const modifier = isMac ? event.metaKey : event.ctrlKey;

  if (modifier && event.key.toLowerCase() === 'w') {
    event.preventDefault();
    if (activeTab.value) {
      requestCloseTab(activeTab.value.id);
    }
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleGlobalKeydown);
  void bootstrap();
});

/**
 * 启动顺序：按系统主题先行渲染 → 读取设置应用主题 → 恢复工作区 → 开启自动保存。
 * 设置读取失败或首次启动时回退到欢迎页，不阻断界面。
 */
async function bootstrap(): Promise<void> {
  appStore.initTheme();

  const settings = await appStore.loadSettings();
  await connectionStore.load();
  void workspaces.load();
  const shouldRestore = settings?.general.restoreWorkspace ?? true;

  const restored = shouldRestore ? await persistence.restore() : false;
  if (!restored) {
    workspaceStore.openWelcomeTab();
  }

  persistence.enable();
  void appStore.loadAppInfo();
}

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleGlobalKeydown);
});
</script>

<template>
  <div class="app-shell">
    <AppMenuBar @action="handleMenuAction" />

    <AppToolbar
      :can-execute="activeTab?.type === 'query' && Boolean(activeTab.context.connectionId)"
      :in-transaction="Boolean(activeTransaction)"
      :workspace-name="workspaces.currentName"
      :workspaces="workspaces.items"
      :current-workspace-id="workspaces.currentId"
      @workspace-select="switchWorkspace"
      @workspace-create="openWorkspaceModal('create')"
      @workspace-rename="openWorkspaceModal('rename')"
      @workspace-delete="handleDeleteWorkspace"
      @execute="queryTabRef?.execute()"
      @begin-transaction="handleBeginTransaction"
      @commit-transaction="handleFinishTransaction('commit')"
      @rollback-transaction="handleFinishTransaction('rollback')"
      @open-settings="settingsVisible = true"
      @open-sql-file="handleOpenSqlFile"
      @save-sql-file="handleSaveSqlFile"
      @create-connection="openCreateDialog"
      @refresh="handleRefresh"
      @toggle-sidebar="uiStore.toggleSidebar()"
      @toggle-result="uiStore.toggleResultPanel()"
    />

    <div class="app-body">
      <AppSidebar
        v-if="uiStore.sidebarVisible"
        @create-connection="openCreateDialog"
        @edit-connection="openEditDialog"
        @delete-connection="handleDeleteConnection"
        @open-table="handleOpenTable"
        @open-structure="handleOpenStructure"
        @show-database-ddl="handleShowDatabaseDdl"
        @new-query="openNewQuery"
        @open-sql="handleOpenSavedSql"
        @open-redis-key="handleOpenRedisKey"
        @open-redis-db="handleOpenRedisDb"
        @close-database="handleCloseDatabase"
      />

      <ResizeHandle
        v-if="uiStore.sidebarVisible"
        axis="x"
        label="调整导航栏宽度"
        @resize="handleSidebarResize"
      />

      <div ref="workspaceRef" class="workspace">
        <WorkspaceTabs
          :tabs="tabs"
          :active-tab-id="activeTabId"
          @activate="workspaceStore.activateTab"
          @close="requestCloseTab"
          @action="handleTabAction"
        />

        <div class="workspace__content">
          <QueryTab
            v-if="activeTab?.type === 'query'"
            :key="activeTab.id"
            ref="queryTabRef"
            :tab="activeTab"
            @update-draft="workspaceStore.updateDraft(activeTab.id, $event)"
          />

          <WelcomeTab
            v-else-if="activeTab?.type === 'welcome'"
            @create-connection="openCreateDialog"
            @new-query="openNewQuery()"
          />

          <TableTab v-else-if="activeTab?.type === 'table'" :key="activeTab.id" :tab="activeTab" />

          <StructureTab
            v-else-if="activeTab?.type === 'structure'"
            :key="activeTab.id"
            :tab="activeTab"
          />

          <RedisKeyTab v-else-if="activeTab?.type === 'redis-key'" :key="activeTab.id" :tab="activeTab" />

          <RedisDbBrowser v-else-if="activeTab?.type === 'redis-db'" :key="activeTab.id" :tab="activeTab" />
        </div>

        <template v-if="uiStore.resultPanelVisible">
          <ResizeHandle axis="y" label="调整结果面板高度" @resize="handleResultResize" />
          <AppResultPanel />
        </template>
      </div>
    </div>

    <SqlLogPanel
      v-if="uiStore.sqlLogVisible"
      @close="uiStore.setSqlLogVisible(false)"
    />

    <AppStatusBar
      v-if="uiStore.statusBarVisible"
      :connection-name="activeTab?.context.connectionName ?? null"
      :database-name="activeTab?.context.databaseName ?? null"
      :engine="activeEngine"
      :sql-log-visible="uiStore.sqlLogVisible"
      :sql-log-count="sqlLogStore.count"
      @toggle-sql-log="uiStore.toggleSqlLog()"
      :tab-count="tabCount"
      :version="appInfo?.version"
    />

    <AppModal v-model="aboutVisible" title="关于 DataDock" :width="440">
      <div class="about">
        <p class="about__slogan">Your Data. One Workspace.</p>
        <dl class="about__list">
          <div>
            <dt>版本</dt>
            <dd>{{ appInfo?.version ?? '—' }}</dd>
          </div>
          <div>
            <dt>Tauri</dt>
            <dd>{{ appInfo?.tauriVersion ?? '—' }}</dd>
          </div>
          <div>
            <dt>运行平台</dt>
            <dd>{{ appInfo ? `${appInfo.os} / ${appInfo.arch}` : '—' }}</dd>
          </div>
          <div>
            <dt>数据目录</dt>
            <dd class="about__path">{{ appInfo?.dataDir ?? '—' }}</dd>
          </div>
          <div>
            <dt>日志目录</dt>
            <dd class="about__path">{{ appInfo?.logDir ?? '—' }}</dd>
          </div>
        </dl>
        <p class="about__note">
          当前为界面骨架阶段，连接管理、SQL 执行与数据浏览将在后续阶段接入。
        </p>
      </div>

      <template #footer>
        <AppButton variant="primary" @click="aboutVisible = false">关闭</AppButton>
      </template>
    </AppModal>

    <SettingsDialog v-model="settingsVisible" />

    <AppModal
      v-model="databaseDdlVisible"
      :title="`建库语句 · ${databaseDdl?.databaseName ?? ''}`"
      :width="720"
    >
      <p v-if="databaseDdlLoading" class="ddl-hint">正在读取建库语句…</p>
      <pre v-else-if="databaseDdl" class="ddl-block">{{ databaseDdl.ddl }}</pre>
      <template #footer>
        <AppButton size="sm" @click="databaseDdlVisible = false">关闭</AppButton>
        <AppButton
          variant="primary"
          size="sm"
          :disabled="!databaseDdl?.ddl"
          @click="copyDatabaseDdl"
        >
          复制语句
        </AppButton>
      </template>
    </AppModal>

    <AppModal
      v-model="workspaceModalVisible"
      :title="workspaceMode === 'create' ? '新建工作区' : '重命名工作区'"
      :width="420"
    >
      <label class="workspace-field">
        <span class="workspace-field__label">名称</span>
        <AppInput
          v-model="workspaceNameInput"
          placeholder="例如：订单库调试"
          size="sm"
          @keyup.enter="submitWorkspaceModal"
        />
      </label>
      <p class="workspace-hint">
        每个工作区拥有独立的标签与布局，切换时当前标签会先保存。
      </p>
      <template #footer>
        <AppButton size="sm" @click="workspaceModalVisible = false">取消</AppButton>
        <AppButton
          variant="primary"
          size="sm"
          :loading="savingWorkspace"
          :disabled="workspaceNameInput.trim().length === 0"
          @click="submitWorkspaceModal"
        >
          {{ workspaceMode === 'create' ? '创建并切换' : '保存' }}
        </AppButton>
      </template>
    </AppModal>

    <ConnectionDialog
      v-model="dialogVisible"
      :mode="dialogMode"
      :connection="editingConnection"
      @saved="handleConnectionSaved"
    />

    <DumpDialog v-model="dumpDialogVisible" :mode="dumpDialogMode" />

    <AppModal
      v-model="confirmVisible"
      :title="confirmPayload?.title ?? '确认操作'"
      :width="420"
      @close="handleCancelConfirm"
    >
      <p>{{ confirmPayload?.message }}</p>

      <template #footer>
        <AppButton @click="handleCancelConfirm">取消</AppButton>
        <AppButton variant="danger" @click="handleConfirm">
          {{ confirmPayload?.confirmText ?? '继续' }}
        </AppButton>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.ddl-hint {
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}

.ddl-block {
  padding: var(--space-3);
  overflow: auto;
  max-height: 60vh;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  line-height: 1.75;
  white-space: pre-wrap;
  word-break: break-word;
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  user-select: text;
}

.workspace-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.workspace-field__label {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.workspace-hint {
  margin-top: var(--space-3);
  color: var(--text-tertiary);
  font-size: 11px;
  line-height: 1.6;
}

.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  /* 用 background 而非 background-color，支持玻璃/拟物主题的渐变背景 */
  background: var(--app-bg);
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.workspace {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.workspace__content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background-color: var(--surface-bg);
}

.about__slogan {
  margin-bottom: var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  letter-spacing: 0.4px;
}

.about__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.about__list > div {
  display: grid;
  grid-template-columns: 76px minmax(0, 1fr);
  gap: var(--space-2);
}

.about__list dt {
  color: var(--text-tertiary);
}

.about__list dd {
  margin: 0;
  color: var(--text-primary);
  word-break: break-all;
}

.about__path {
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

.about__note {
  margin-top: var(--space-4);
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
}
</style>
