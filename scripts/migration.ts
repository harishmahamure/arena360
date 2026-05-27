import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const BACKEND = join(ROOT, 'apps/backend');

const usage = `Usage:
  pnpm migration generate <description>   Create a new SQL migration file
  pnpm migration run                      Apply pending migrations
  pnpm migration revert                   Revert the latest migration
  pnpm migration info                     Show migration status
  pnpm migration prepare                  Generate sqlx offline query data

Examples:
  pnpm migration generate add_audit_columns
  pnpm migration run
  pnpm migration prepare`;

function run(command: string, args: string[]): number {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.log(`$ ${command} ${args.join(' ')}`);
  const result = spawnSync(command, args, {
    cwd: BACKEND,
    stdio: 'inherit',
    env: process.env,
  });

  if (result.error) {
    // biome-ignore lint/suspicious/noConsole: CLI script
    console.error(result.error.message);
    return 1;
  }

  return result.status ?? 1;
}

function ensureBackend(): void {
  if (!existsSync(BACKEND)) {
    // biome-ignore lint/suspicious/noConsole: CLI script
    console.error(`Backend directory not found at ${BACKEND}`);
    process.exit(1);
  }
}

const [, , command, ...rest] = process.argv;

ensureBackend();

switch (command) {
  case 'generate': {
    const description = rest.join('_').trim();
    if (!description) {
      // biome-ignore lint/suspicious/noConsole: CLI script
      console.error('Missing migration description.\n');
      // biome-ignore lint/suspicious/noConsole: CLI script
      console.error(usage);
      process.exit(1);
    }
    process.exit(run('sqlx', ['migrate', 'add', description]));
    break;
  }
  case 'run': {
    process.exit(run('sqlx', ['migrate', 'run']));
    break;
  }
  case 'revert': {
    process.exit(run('sqlx', ['migrate', 'revert']));
    break;
  }
  case 'info': {
    process.exit(run('sqlx', ['migrate', 'info']));
    break;
  }
  case 'prepare': {
    process.exit(run('cargo', ['sqlx', 'prepare', '--workspace']));
    break;
  }
  default: {
    // biome-ignore lint/suspicious/noConsole: CLI script
    console.error(usage);
    process.exit(command ? 1 : 0);
  }
}
