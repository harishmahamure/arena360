import path from 'node:path';
import react from '@vitejs/plugin-react';
import tsconfigPaths from 'vite-tsconfig-paths';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  resolve: {
    alias: {
      '@gaming-cafe/ui': path.resolve(__dirname, '../../packages/ui/src/index.ts'),
      '@gaming-cafe/utils': path.resolve(__dirname, '../../packages/utils/src/index.ts'),
      '@gaming-cafe/theme': path.resolve(__dirname, '../../packages/theme/src/index.ts'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/testSetup.ts'],
    maxWorkers: 2,
    minWorkers: 1,
    testTimeout: 15_000,
  },
});
