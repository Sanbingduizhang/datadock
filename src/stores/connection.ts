import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import {
  connectDatabase,
  createConnection,
  deleteConnection,
  disconnectDatabase,
  exportConnections,
  importConnections,
  listConnections,
  testConnection,
  updateConnection,
} from '@/api/connection';
import { useToast } from '@/composables/useToast';
import type { AppError } from '@/types/api';
import type {
  ConnectionExportFile,
  ConnectionInfo,
  CreateConnectionRequest,
  TestConnectionRequest,
  TestConnectionResponse,
  UpdateConnectionRequest,
} from '@/types/connection';
import { normalizeError } from '@/utils/errors';

/**
 * 连接配置与运行时状态。
 * 密码不进入前端状态，只在创建/编辑时提交给后端。
 */
export const useConnectionStore = defineStore('connection', () => {
  const connections = ref<ConnectionInfo[]>([]);
  const loading = ref(false);
  const loaded = ref(false);
  const error = ref<AppError | null>(null);
  const busyConnectionId = ref<string | null>(null);

  const toast = useToast();

  const connectedCount = computed(
    () => connections.value.filter((connection) => connection.connected).length,
  );

  function byId(connectionId: string): ConnectionInfo | undefined {
    return connections.value.find((connection) => connection.id === connectionId);
  }

  async function load(): Promise<void> {
    loading.value = true;
    error.value = null;

    try {
      connections.value = await listConnections();
      loaded.value = true;
    } catch (raw) {
      error.value = normalizeError(raw);
      toast.error(`读取连接列表失败：${error.value.message}`);
    } finally {
      loading.value = false;
    }
  }

  async function create(request: CreateConnectionRequest): Promise<string | null> {
    try {
      const response = await createConnection(request);
      await load();
      toast.success('连接已保存');
      return response.connectionId;
    } catch (raw) {
      toast.error(`保存连接失败：${normalizeError(raw).message}`);
      return null;
    }
  }

  async function update(request: UpdateConnectionRequest): Promise<boolean> {
    try {
      await updateConnection(request);
      await load();
      toast.success('连接已更新，请重新连接');
      return true;
    } catch (raw) {
      toast.error(`更新连接失败：${normalizeError(raw).message}`);
      return false;
    }
  }

  async function remove(connectionId: string): Promise<boolean> {
    try {
      await deleteConnection(connectionId);
      connections.value = connections.value.filter((item) => item.id !== connectionId);
      toast.success('连接已删除');
      return true;
    } catch (raw) {
      toast.error(`删除连接失败：${normalizeError(raw).message}`);
      return false;
    }
  }

  async function test(request: TestConnectionRequest): Promise<TestConnectionResponse | null> {
    try {
      return await testConnection(request);
    } catch (raw) {
      toast.error(`测试连接失败：${normalizeError(raw).message}`);
      return null;
    }
  }

  async function connect(connectionId: string): Promise<boolean> {
    busyConnectionId.value = connectionId;
    try {
      const response = await connectDatabase(connectionId);
      const target = byId(connectionId);
      if (target) {
        target.connected = true;
        target.serverVersion = response.serverVersion ?? null;
      }
      const engine =
        target?.sourceType === 'redis'
          ? 'Redis'
          : target?.sourceType === 'postgres'
            ? 'PostgreSQL'
            : 'MySQL';
      toast.success(
        `已连接${response.serverVersion ? `（${engine} ${response.serverVersion}）` : ''}`,
      );
      return true;
    } catch (raw) {
      toast.error(`连接失败：${normalizeError(raw).message}`);
      return false;
    } finally {
      busyConnectionId.value = null;
    }
  }

  async function disconnect(connectionId: string): Promise<boolean> {
    busyConnectionId.value = connectionId;
    try {
      await disconnectDatabase(connectionId);
      const target = byId(connectionId);
      if (target) {
        target.connected = false;
      }
      return true;
    } catch (raw) {
      toast.error(`断开连接失败：${normalizeError(raw).message}`);
      return false;
    } finally {
      busyConnectionId.value = null;
    }
  }

  /** 导出全部连接配置（JSON，不含密码），返回可写盘的文件结构 */
  async function exportAll(): Promise<ConnectionExportFile | null> {
    try {
      const file = await exportConnections();
      toast.success(`已导出 ${file.connections.length} 个连接配置`);
      return file;
    } catch (raw) {
      toast.error(`导出连接失败：${normalizeError(raw).message}`);
      return null;
    }
  }

  /** 导入连接配置（JSON 文本），返回导入数量（失败返回 null） */
  async function importAll(json: string): Promise<number | null> {
    try {
      const response = await importConnections(json);
      await load();
      const { imported, skipped } = response;
      toast.success(
        skipped > 0
          ? `已导入 ${imported} 个连接，跳过 ${skipped} 个（重名或无效）`
          : `已导入 ${imported} 个连接`,
      );
      return imported;
    } catch (raw) {
      toast.error(`导入连接失败：${normalizeError(raw).message}`);
      return null;
    }
  }

  return {
    connections,
    loading,
    loaded,
    error,
    busyConnectionId,
    connectedCount,
    byId,
    load,
    create,
    update,
    remove,
    test,
    connect,
    disconnect,
    exportAll,
    importAll,
  };
});
