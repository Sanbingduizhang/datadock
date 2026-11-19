<script setup lang="ts">
import { Info, Plus, SquareTerminal } from 'lucide-vue-next';

import AppButton from '@/components/common/AppButton.vue';

/** 欢迎页：无标签时的起点，提供最常用的两个入口 */
const emit = defineEmits<{
  'create-connection': [];
  'new-query': [];
}>();
</script>

<template>
  <section class="welcome">
    <div class="welcome__inner">
      <div class="welcome__brand">
        <span class="welcome__mark" aria-hidden="true"><i></i><i></i><i></i></span>
        <h1 class="welcome__title">DataDock</h1>
        <p class="welcome__slogan">Your Data. One Workspace.</p>
        <p class="welcome__desc">
          在一个工作台中管理 MySQL 连接、浏览数据库对象、执行查询并维护数据。
        </p>
      </div>

      <div class="welcome__actions">
        <AppButton variant="primary" @click="emit('create-connection')">
          <Plus :size="15" />
          新建连接
        </AppButton>
        <AppButton @click="emit('new-query')">
          <SquareTerminal :size="15" />
          新建 SQL 查询
        </AppButton>
      </div>

      <p class="welcome__hint">
        <Info :size="13" />
        当前为界面骨架阶段：左侧为示例结构，连接管理与 SQL 执行将在后续阶段接入。
      </p>
    </div>
  </section>
</template>

<style scoped>
.welcome {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: var(--space-8);
  overflow: auto;
}

.welcome__inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-6);
  max-width: 460px;
  text-align: center;
}

.welcome__brand {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
}

.welcome__mark {
  display: flex;
  align-items: flex-end;
  gap: 4px;
  height: 34px;
  margin-bottom: var(--space-2);
}

.welcome__mark i {
  display: block;
  width: 6px;
  background-color: var(--primary-color);
  border-radius: 3px;
}

.welcome__mark i:nth-child(1) {
  height: 14px;
  opacity: 0.55;
}

.welcome__mark i:nth-child(2) {
  height: 30px;
}

.welcome__mark i:nth-child(3) {
  height: 21px;
  opacity: 0.75;
}

.welcome__title {
  font-size: 26px;
  font-weight: 600;
  letter-spacing: 0.4px;
}

.welcome__slogan {
  color: var(--text-tertiary);
  font-size: var(--font-size-sm);
  letter-spacing: 0.6px;
  text-transform: uppercase;
}

.welcome__desc {
  max-width: 380px;
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: 1.7;
}

.welcome__actions {
  display: flex;
  gap: var(--space-2);
}

.welcome__hint {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 420px;
  color: var(--text-tertiary);
  font-size: var(--font-size-xs);
  line-height: 1.6;
}
</style>
