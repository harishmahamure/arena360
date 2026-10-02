import react from '@vitejs/plugin-react';
import tsconfigPaths from 'vite-tsconfig-paths';
import { defineConfig } from 'vitest/config';
import { workspaceAliases } from './workspaceAliases';

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  resolve: { alias: workspaceAliases },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: [],
  },
});
