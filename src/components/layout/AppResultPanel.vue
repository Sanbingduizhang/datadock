<script setup lang="ts">
import { ChevronDown, ListTree, MessageSquare, Table2 } from 'lucide-vue-next';
import { ref } from 'vue';

import AppEmpty from '@/components/common/AppEmpty.vue';
import AppIconButton from '@/components/common/AppIconButton.vue';

import { useUiStore } from '@/stores/ui';

/**
 * 底部结果面板：结果 / 消息 / 执行日志。
 * 阶段 B 只有面板骨架与空状态，阶段 E 接入真实执行结果。
 */
type ResultTabKey = 'result' | 'message' | 'log';

const uiStore = useUiStore();
const activeKey = ref<ResultTabKey>('result');

const TABS = [
  { key: 'result' as const, label: '结果', icon: Table2 },
  { key: 'message' as const, label: '消息', icon: MessageSquare },
  { key: 'log' as const, label: '执行日志', icon: ListTree },
];
</script>

<template>
  <section class="result-panel" :style="{ height: `${uiStore.resultPanelHeight}px` }">
    <header class="result-panel__head">
      <div class="result-tabs" role="tablist" aria-label="查询结果">
        <button
          v-for="tab in TABS"
          :key="tab.key"
          type="button"
          role="tab"
          class="result-tab"
          :class="{ 'is-active': activeKey === tab.key }"
          :aria-selected="activeKey === tab.key"
          @click="activeKey = tab.key"
        >
          <component :is="tab.icon" :size="13" />
          {{ tab.label }}
        </button>
      </div>

      <div class="result-panel__actions">
        <AppIconButton label="收起结果面板" @click="uiStore.toggleResultPanel()">
          <ChevronDown :size="15" />
        </AppIconButton>
      </div>
    </header>

    <div class="result-panel__body">
      <AppEmpty
        v-if="activeKey === 'result'"
        title="尚未执行 SQL"
        description="执行查询后，结果集会展示在这里；支持多结果集切换与大结果集分批加载（阶段 E 接入）。"
      />
      <AppEmpty
        v-else-if="activeKey === 'message'"
        title="暂无执行消息"
        description="执行成功后这里会显示影响行数、执行耗时等信息。"
        compact
      />
      <AppEmpty
        v-else
        title="暂无执行日志"
        description="执行日志会记录本次执行的 SQL、开始时间、耗时与执行状态。"
        compact
      />
    </div>
  </section>
</template>

<style scoped>
.result-panel {
  display: flex;
  flex: none;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background-color: var(--surface-bg);
  border-top: 1px solid var(--border-color);
}

.result-panel__head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  height: 32px;
  padding: 0 var(--space-1) 0 var(--space-2);
  border-bottom: 1px solid var(--divider-color);
}

.result-tabs {
  display: flex;
  align-items: stretch;
  gap: 2px;
  height: 100%;
}

.result-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 0 var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  border-bottom: 2px solid transparent;
  transition:
    color var(--transition-fast),
    border-color var(--transition-fast);
}

.result-tab:hover {
  color: var(--text-primary);
}

.result-tab.is-active {
  color: var(--text-primary);
  border-bottom-color: var(--primary-color);
}

.result-panel__actions {
  margin-left: auto;
}

.result-panel__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
</style>
