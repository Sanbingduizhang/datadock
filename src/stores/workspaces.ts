import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import { deleteWorkspace, listWorkspaces } from '@/api/workspace';

import type { WorkspaceSummary } from '@/types/workspace';

const CURRENT_KEY = 'datadock.workspaceId';
export const DEFAULT_WORKSPACE_ID = 'main';

function readCurrentId(): string {
  try {
    return localStorage.getItem(CURRENT_KEY) || DEFAULT_WORKSPACE_ID;
  } catch {
    return DEFAULT_WORKSPACE_ID;
  }
}

/**
 * 多工作区状态：记住当前工作区并提供列表。
 *
 * 当前工作区 ID 存在 localStorage（窗口级偏好），
 * 标签与布局仍由 `workspace_state` 表按 ID 持久化。
 */
export const useWorkspacesStore = defineStore('workspaces', () => {
  const currentId = ref(readCurrentId());
  const currentName = ref(currentId.value === DEFAULT_WORKSPACE_ID ? '默认工作区' : currentId.value);
  const items = ref<WorkspaceSummary[]>([]);
  const loading = ref(false);

  const current = computed(
    () => items.value.find((item) => item.id === currentId.value) ?? null,
  );

  function setCurrent(id: string, name?: string): void {
    currentId.value = id;
    currentName.value = name ?? (id === DEFAULT_WORKSPACE_ID ? '默认工作区' : id);
    try {
      localStorage.setItem(CURRENT_KEY, id);
    } catch {
      // localStorage 不可用时仅当前会话生效
    }
  }

  function renameCurrent(name: string): void {
    currentName.value = name;
  }

  async function load(): Promise<void> {
    loading.value = true;
    try {
      items.value = await listWorkspaces();
      const matched = items.value.find((item) => item.id === currentId.value);
      if (matched) {
        currentName.value = matched.name;
      }
    } finally {
      loading.value = false;
    }
  }

  async function remove(id: string): Promise<void> {
    await deleteWorkspace(id);
    items.value = items.value.filter((item) => item.id !== id);
  }

  return {
    currentId,
    currentName,
    items,
    loading,
    current,
    setCurrent,
    renameCurrent,
    load,
    remove,
  };
});
