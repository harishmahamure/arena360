#!/usr/bin/env tsx
// Generates packages/api-types/src/schema.ts from packages/api-types/openapi.json
// Pipeline: regenerate backend spec -> run openapi-typescript.
import { execSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

const ROOT = process.cwd();
const TOOLS = join(ROOT, 'tools/backend-cli');
const SPEC = join(ROOT, 'packages/api-types/openapi.json');
const OUT = join(ROOT, 'packages/api-types/src/schema.ts');

function run(cmd: string, cwd = ROOT): void {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.log(`$ ${cmd}`);
  execSync(cmd, { cwd, stdio: 'inherit' });
}

if (!existsSync(TOOLS)) {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error(`tools/backend-cli not found at ${TOOLS}. Skipping spec regeneration.`);
} else {
  run('cargo run -p arena360-tools --bin openapi-gen --quiet');
}

if (!existsSync(SPEC)) {
  throw new Error(`OpenAPI spec not found at ${SPEC}`);
}
mkdirSync(dirname(OUT), { recursive: true });
run(`pnpm dlx openapi-typescript@^7 ${SPEC} -o ${OUT}`);
// biome-ignore lint/suspicious/noConsole: CLI script
console.log(`api-types written to ${OUT}`);
