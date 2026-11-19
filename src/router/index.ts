import { createRouter, createWebHashHistory } from 'vue-router';

import WorkspaceView from '@/views/workspace/WorkspaceView.vue';

/**
 * DataDock 是桌面应用，路由只负责最外层页面切换。
 * 工作区内部的标签页（SQL 编辑器、数据浏览、表结构）由工作区状态管理，不占用路由。
 * 使用 hash 模式，避免打包后 file 协议下的路径问题。
 */
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'workspace',
      component: WorkspaceView,
      meta: { title: '工作区' },
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/',
    },
  ],
});

export default router;
