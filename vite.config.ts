import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { TanStackRouterVite } from '@tanstack/router-plugin/vite'
import path from 'node:path'

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [TanStackRouterVite({ target: 'react', autoCodeSplitting: true }), react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  // Vite options tailored for Tauri development and only applied using `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  //    端口 1421 落在本机 Windows 系统排除区间（netsh excludedportrange 1376-1475），
  //    绑定会报 EACCES；改用 Vite 默认安全端口 5173（需与 tauri.conf.json 的 devUrl 同步）
  server: {
    port: 5173,
    strictPort: true,
    host: '127.0.0.1',
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
}))
