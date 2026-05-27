#!/usr/bin/env tsx
// Generates packages/api-types/src/schema.ts from apps/backend/docs/openapi.json
// Pipeline: regenerate backend spec -> run openapi-typescript.
import { execSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

const ROOT = process.cwd();
const BACKEND = join(ROOT, 'apps/backend');
const SPEC = join(BACKEND, 'docs/openapi.json');
const OUT = join(ROOT, 'packages/api-types/src/schema.ts');

function run(cmd: string, cwd = ROOT): void {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.log(`$ ${cmd}`);
  execSync(cmd, { cwd, stdio: 'inherit' });
}

if (!existsSync(BACKEND)) {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error(`apps/backend not found at ${BACKEND}. Skipping spec regeneration.`);
} else {
  run('cargo run --bin openapi-gen --quiet', BACKEND);
}

if (!existsSync(SPEC)) {
  throw new Error(`OpenAPI spec not found at ${SPEC}`);
}
mkdirSync(dirname(OUT), { recursive: true });
run(`pnpm dlx openapi-typescript@^7 ${SPEC} -o ${OUT}`);
// biome-ignore lint/suspicious/noConsole: CLI script
console.log(`api-types written to ${OUT}`);
