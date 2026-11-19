import { fileURLToPath, URL } from 'node:url';

import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';

// DataDock 前端构建配置。
// 开发服务器端口固定为 5173，与 src-tauri/tauri.conf.json 中的 devUrl 保持一致。
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // Tauri 使用 Chromium/WebKit 内核，目标可按现代浏览器处理
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    // Monaco 单独成块（按需加载），放宽单块体积告警
    chunkSizeWarningLimit: 4096,
  },
});
