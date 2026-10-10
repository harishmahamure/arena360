import { spawnSync } from 'node:child_process';
import { readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../', import.meta.url));
const CORE = join(ROOT, 'crates/backend-core');
const ENV_DIR = join(ROOT, 'apps/backend');
const usage = `Usage:
  pnpm migration generate <description> [--target control|tenant]
  pnpm migration run [--target control]
  pnpm migration info [--target control]
  pnpm migration revert [--target control]
  pnpm migration prepare [--target control]

CONTROL_DATABASE_URL explicitly selects the PostgreSQL control plane.
Tenant migrations are applied through the lease-fenced tenant migration orchestrator.
The shared operational PostgreSQL schema is retired.`;
const args = process.argv.slice(2);
const targetIndex = args.indexOf('--target');
let target = 'control';
if (targetIndex >= 0) {
  target = args[targetIndex + 1];
  args.splice(targetIndex, 2);
}
const [command, ...rest] = args;
function fail(message: string): never {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
if (!['control', 'tenant'].includes(target)) fail('--target must be control or tenant');
if (!command || command === '--help') {
  process.stdout.write(`${usage}\n`);
  process.exit(0);
}
const source = join(CORE, 'migrations', target);
if (command === 'generate') {
  const description = rest.join('_');
  if (!/^[a-z][a-z0-9_]*$/.test(description))
    fail('Use a lowercase underscore-separated description');
  const versions = readdirSync(source).map((name) =>
    Number(name.match(/^(\d+)_.*\.sql$/)?.[1] ?? 0),
  );
  const version = Math.max(0, ...versions) + 1;
  const path = join(source, `${String(version).padStart(4, '0')}_${description}.sql`);
  writeFileSync(path, `-- ${description.replaceAll('_', ' ')}\n`, { flag: 'wx' });
  process.stdout.write(`Created ${path}\n`);
  process.exit(0);
}
if (target === 'tenant')
  fail(
    'Tenant migrations require the owning cell, a current lease, and the tenant migration orchestrator',
  );
if (!['run', 'info', 'revert', 'prepare'].includes(command) || rest.length) fail(usage);
for (const directory of [ROOT, ENV_DIR]) {
  try {
    process.loadEnvFile(join(directory, '.env'));
    break;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
}
const url = process.env.CONTROL_DATABASE_URL;
if (!url) fail('Set CONTROL_DATABASE_URL; DATABASE_URL is not a migration target');
const result = spawnSync(
  command === 'prepare' ? 'cargo' : 'sqlx',
  command === 'prepare'
    ? ['sqlx', 'prepare', '--workspace']
    : ['migrate', command, '--source', source],
  { cwd: CORE, env: { ...process.env, DATABASE_URL: url }, stdio: 'inherit' },
);
if (result.error) fail(result.error.message);
process.exit(result.status ?? 1);
