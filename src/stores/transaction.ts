import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

import {
  beginTransaction,
  commitTransaction,
  rollbackTransaction,
} from '@/api/transaction';

import type { TransactionResponse } from '@/types/transaction';

export interface TabTransaction {
  transactionId: string;
  connectionId: string;
  databaseName: string | null;
  startedAt: number;
}

/**
 * 手动事务状态：按「查询标签」维度保存。
 *
 * 事务会话在后端与连接绑定，前端只保存 ID；
 * 标签关闭或连接断开时通过 rollback 释放，避免悬挂事务。
 */
export const useTransactionStore = defineStore('transaction', () => {
  const entries = ref<Record<string, TabTransaction>>({});

  const activeCount = computed(() => Object.keys(entries.value).length);

  function get(tabId: string): TabTransaction | null {
    return entries.value[tabId] ?? null;
  }

  function has(tabId: string): boolean {
    return Boolean(entries.value[tabId]);
  }

  function drop(tabId: string): void {
    const next = { ...entries.value };
    delete next[tabId];
    entries.value = next;
  }

  async function begin(
    tabId: string,
    connectionId: string,
    databaseName?: string | null,
  ): Promise<void> {
    const reply = await beginTransaction({
      connectionId,
      ...(databaseName ? { databaseName } : {}),
    });

    entries.value = {
      ...entries.value,
      [tabId]: {
        transactionId: reply.transactionId,
        connectionId,
        databaseName: databaseName ?? null,
        startedAt: Date.now(),
      },
    };
  }

  async function commit(tabId: string): Promise<string> {
    const entry = entries.value[tabId];
    if (!entry) {
      throw new Error('当前标签没有进行中的事务');
    }

    const reply: TransactionResponse = await commitTransaction({
      connectionId: entry.connectionId,
      transactionId: entry.transactionId,
    });

    drop(tabId);
    return reply.message;
  }

  async function rollback(tabId: string): Promise<string> {
    const entry = entries.value[tabId];
    if (!entry) {
      throw new Error('当前标签没有进行中的事务');
    }

    const reply = await rollbackTransaction({
      connectionId: entry.connectionId,
      transactionId: entry.transactionId,
    });

    drop(tabId);
    return reply.message;
  }

  /** 回滚某个连接下的全部事务（连接断开时使用） */
  async function rollbackByConnection(connectionId: string): Promise<void> {
    const targets = Object.entries(entries.value).filter(
      ([, entry]) => entry.connectionId === connectionId,
    );

    await Promise.allSettled(
      targets.map(([tabId]) => rollback(tabId).catch(() => drop(tabId))),
    );
  }

  /** 后端会话已失效时仅清理前端状态（例如连接被删除） */
  function forget(tabId: string): void {
    drop(tabId);
  }

  return {
    entries,
    activeCount,
    get,
    has,
    begin,
    commit,
    rollback,
    rollbackByConnection,
    forget,
  };
});
