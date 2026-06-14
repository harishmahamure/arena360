import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    environmentMatchGlobs: [
      ['src/lib/session-clock/**/*.test.ts', 'jsdom'],
      ['src/lib/hooks/**/*.test.ts', 'jsdom'],
    ],
    include: ['src/**/*.test.ts'],
  },
});
