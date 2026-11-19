import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vitest/config';

// 前端单元测试配置。当前仅覆盖纯逻辑模块（工具函数、类型转换），
// 组件级测试后续随 UI 阶段引入。
export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    environment: 'node',
    include: ['tests/frontend/**/*.test.ts'],
  },
});
