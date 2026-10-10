import path from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  resolve: {
    alias: {
      '@gaming-cafe/ui': path.resolve(__dirname, '../../packages/ui/src/index.ts'),
      '@gaming-cafe/utils': path.resolve(__dirname, '../../packages/utils/src/index.ts'),
      '@gaming-cafe/theme': path.resolve(__dirname, '../../packages/theme/src/index.ts'),
    },
  },
  build: { outDir: 'dist', emptyOutDir: true },
  server: {
    port: 5174,
    proxy: {
      '/platform': {
        target: process.env.PLATFORM_API_PROXY_TARGET ?? 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
});
