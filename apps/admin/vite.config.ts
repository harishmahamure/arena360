import path from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig(({ mode }) => ({
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
      input: './index.html',
    },
    minify: 'esbuild',
  },
  esbuild: {
    drop: mode === 'production' ? ['console', 'debugger'] : [],
  },
  resolve: {
    alias: {
      '@gaming-cafe/ui': path.resolve(__dirname, '../../packages/ui/src'),
      '@gaming-cafe/theme': path.resolve(__dirname, '../../packages/theme/src'),
      '@gaming-cafe/providers': path.resolve(__dirname, '../../packages/providers/src'),
      '@gaming-cafe/utils': path.resolve(__dirname, '../../packages/utils/src'),
    },
  },
  server: {
    port: 5173,
    open: true,
  },
}));
