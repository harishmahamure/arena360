import path from 'node:path';

// Resolve workspace source in both the app and tests; generated dist can be stale.
export const workspaceAliases = Object.fromEntries(
  ['ui', 'theme', 'providers', 'utils', 'contracts', 'proto', 'api-types'].map((name) => [
    `@gaming-cafe/${name}`,
    path.resolve(__dirname, `../../packages/${name}/src`),
  ]),
);
