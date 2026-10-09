import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';
import { workspaceAliases } from './workspaceAliases';

export default defineConfig(({ mode }) => ({
  base: process.env.VITE_BASE_PATH ?? '/',
  plugins: [
    react(),
    tsconfigPaths({
      projects: ['./tsconfig.json'],
    }),
  ],
  root: './',
  publicDir: './public',
  build: {
    outDir: './dist',
    emptyOutDir: true,
    sourcemap: false,
    rollupOptions: {
      input: { admin: './index.html', portal: './portal.html' },
    },
    minify: 'esbuild',
  },
  esbuild: {
    drop: mode === 'production' ? ['console', 'debugger'] : [],
  },
  resolve: {
    alias: workspaceAliases,
  },
  server: {
    port: 5173,
    open: false,
    proxy: {
      '/platform': {
        target: process.env.PLATFORM_API_PROXY_TARGET ?? 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
}));
