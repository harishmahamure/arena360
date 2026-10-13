import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../', import.meta.url));
function run(args, { envFile = false, exitCode = 0 } = {}) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'arena-migration-cli-')));
  try {
    for (const directory of ['scripts', 'bin', 'crates/backend-core'])
      mkdirSync(join(root, directory), { recursive: true });
    copyFileSync(join(repo, 'scripts/migration.ts'), join(root, 'scripts/migration.ts'));
    writeFileSync(join(root, 'package.json'), '{"type":"module"}');
    const capture = join(root, 'capture.json');
    const stub = `#!${process.execPath}\nimport {writeFileSync} from 'node:fs';writeFileSync(process.env.MIGRATION_CAPTURE, JSON.stringify({args:process.argv.slice(2),cwd:process.cwd(),url:process.env.DATABASE_URL}));process.exit(${exitCode});\n`;
    for (const program of ['cargo', 'sqlx'])
      writeFileSync(join(root, 'bin', program), stub, { mode: 0o755 });
    const env = {
      ...process.env,
      PATH: `${join(root, 'bin')}:${process.env.PATH}`,
      MIGRATION_CAPTURE: capture,
    };
    delete env.CONTROL_DATABASE_URL;
    const url = 'postgres://migration-test@localhost:1/control';
    if (envFile) writeFileSync(join(root, '.env'), `CONTROL_DATABASE_URL=${url}\n`);
    else env.CONTROL_DATABASE_URL = url;
    const result = spawnSync(
      process.execPath,
      ['--import', 'tsx', join(root, 'scripts/migration.ts'), ...args],
      { cwd: repo, env, encoding: 'utf8' },
    );
    return {
      ...result,
      invocation: existsSync(capture) ? JSON.parse(readFileSync(capture, 'utf8')) : null,
      core: join(root, 'crates/backend-core'),
      url,
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('tenant run dispatches all tenants to the rollout operator and waits', () => {
  const result = run(['run', '--target', 'tenant'], { envFile: true });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.invocation.args, [
    'run',
    '-p',
    'arena360-tools',
    '--bin',
    'schema_rollout',
    '--',
    '--all',
    '--wait-seconds',
    '600',
  ]);
  assert.equal(result.invocation.cwd, result.core);
  assert.equal(result.invocation.url, result.url);
});
test('tenant timeout and operator failures propagate', () => {
  const result = run(['run', '--target', 'tenant', '--timeout-seconds', '1200'], { exitCode: 7 });
  assert.equal(result.status, 7, result.stderr);
  assert.equal(result.invocation.args.at(-1), '1200');
});
test('invalid tenant arguments never dispatch a migration', () => {
  for (const args of [
    ['run', '--target', 'tenant', '--timeout-seconds', '0'],
    ['run', '--target', 'tenant', '--unknown'],
    ['revert', '--target', 'tenant'],
  ]) {
    const result = run(args);
    assert.equal(result.status, 1);
    assert.equal(result.invocation, null);
  }
});
test('control run still selects only the PostgreSQL control migrations', () => {
  const result = run(['run', '--target', 'control']);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.invocation.args, [
    'migrate',
    'run',
    '--source',
    join(result.core, 'migrations/control'),
  ]);
});
