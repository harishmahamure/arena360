#!/usr/bin/env node
/** Run SQLite tests and control-plane tests without using an application database. */
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { access, mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import pg from 'pg';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const manifest = join(root, 'apps/backend/Cargo.toml');
const controlTargets = [
  'control_plane',
  'ownership_fencing',
  'tenant_bootstrap',
  'tenant_user_repositories',
  'demo_seed',
];
let activeChild;
let interrupted = false;
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    interrupted = true;
    activeChild?.kill(signal);
  });
}
async function command(program, args, options = {}) {
  if (interrupted && !options.cleanup) throw new Error('Integration run interrupted');
  const child = spawn(program, args, {
    cwd: root,
    env: options.env ?? process.env,
    stdio: options.capture ? ['ignore', 'pipe', 'pipe'] : 'inherit',
  });
  activeChild = child;
  let output = '';
  if (options.capture) {
    child.stdout.on('data', (chunk) => {
      output += chunk;
    });
    child.stderr.resume();
  }
  try {
    await new Promise((resolve, reject) => {
      child.on('error', reject);
      child.on('close', (code) =>
        code === 0 ? resolve() : reject(new Error(`${program} failed (${code ?? 'interrupted'})`)),
      );
    });
    return output.trim();
  } finally {
    if (activeChild === child) activeChild = undefined;
  }
}
async function postgresBin() {
  const candidates = [];
  if (process.env.PG_BINDIR) candidates.push(process.env.PG_BINDIR);
  try {
    candidates.push(await command('pg_config', ['--bindir'], { capture: true }));
  } catch {
    /* PATH may only contain Rust/Node. */
  }
  for (const bin of candidates) {
    try {
      await access(join(bin, 'initdb'));
      await access(join(bin, 'pg_ctl'));
      return bin;
    } catch {
      /* Try the next installed runtime. */
    }
  }
  throw new Error(
    'Install local PostgreSQL server tools (or set PG_BINDIR), or set CONTROL_TEST_ADMIN_DATABASE_URL for a server that permits disposable database creation',
  );
}
async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const port = server.address().port;
  await new Promise((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  return port;
}
const shellQuote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
async function localPostgres() {
  const bin = await postgresBin();
  const directory = await mkdtemp(join(tmpdir(), 'arena360-test-control-'));
  const data = join(directory, 'cluster');
  let started = false;
  const cleanup = async () => {
    if (started)
      await command(join(bin, 'pg_ctl'), ['-D', data, '-m', 'fast', '-w', 'stop'], {
        cleanup: true,
        capture: true,
      });
    await rm(directory, { recursive: true, force: true });
  };
  try {
    await command(
      join(bin, 'initdb'),
      ['-D', data, '--username=arena_test', '--auth=trust', '--encoding=UTF8', '--no-locale'],
      { capture: true },
    );
    const port = await freePort();
    await command(
      join(bin, 'pg_ctl'),
      [
        '-D',
        data,
        '-l',
        join(directory, 'postgres.log'),
        '-o',
        `-h 127.0.0.1 -p ${port} -k ${shellQuote(directory)}`,
        '-w',
        'start',
      ],
      { capture: true },
    );
    started = true;
    return { url: `postgres://arena_test@127.0.0.1:${port}/postgres`, cleanup };
  } catch (error) {
    await cleanup();
    throw error;
  }
}
async function main() {
  const args = process.argv.slice(2);
  if (args.some((arg) => arg !== '--control-only'))
    throw new Error('Usage: pnpm backend:test:integration [--control-only]');
  const local = process.env.CONTROL_TEST_ADMIN_DATABASE_URL ? undefined : await localPostgres();
  const admin = new pg.Client({
    connectionString: process.env.CONTROL_TEST_ADMIN_DATABASE_URL || local.url,
  });
  const name = `arena360_test_${randomUUID().replaceAll('-', '')}`;
  let created = false;
  let tenantFiles;
  try {
    tenantFiles = await mkdtemp(join(tmpdir(), 'arena360-test-tenants-'));
    await admin.connect();
    await admin.query(`CREATE DATABASE "${name}" TEMPLATE template0`);
    created = true;
    const url = new URL(process.env.CONTROL_TEST_ADMIN_DATABASE_URL || local.url);
    url.pathname = `/${name}`;
    const env = {
      ...process.env,
      CONTROL_TEST_DATABASE_URL: url.toString(),
      CONTROL_DATABASE_URL: url.toString(),
      DATABASE_URL:
        'postgres://invalid:invalid@127.0.0.1:1/operational_database_is_not_a_test_fixture',
      NATS_URL: '',
      REDIS_URL: '',
      CLICKHOUSE_URL: '',
      TENANT_DATA_DIR: tenantFiles,
      TMPDIR: tenantFiles,
      TMP: tenantFiles,
      TEMP: tenantFiles,
    };
    process.stdout.write(
      '[backend integration] Temporary control database created; tenant tests use isolated SQLite files.\n',
    );
    if (!args.includes('--control-only'))
      await command('cargo', ['test', '--manifest-path', manifest], { env });
    // Only control targets include ignored tests; external analytics/Redis suites retain their explicit gates.
    const targets = controlTargets.flatMap((target) => ['--test', target]);
    await command(
      'cargo',
      ['test', '--manifest-path', manifest, ...targets, '--', '--ignored', '--test-threads=1'],
      { env },
    );
    process.stdout.write('[backend integration] Requested checks passed.\n');
  } finally {
    try {
      if (created) {
        await admin.query(
          'SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname=$1 AND pid<>pg_backend_pid()',
          [name],
        );
        await admin.query(`DROP DATABASE "${name}"`);
        process.stdout.write('[backend integration] Temporary control database removed.\n');
      }
    } finally {
      try {
        await admin.end();
      } finally {
        try {
          if (tenantFiles) await rm(tenantFiles, { recursive: true, force: true });
        } finally {
          await local?.cleanup();
        }
      }
    }
  }
}
main().catch((error) => {
  process.stderr.write(`Backend integration failed: ${error.message}\n`);
  process.exitCode = interrupted ? 130 : 1;
});
