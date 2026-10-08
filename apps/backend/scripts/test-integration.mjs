#!/usr/bin/env node
// Fixtures exercise disk zones explicitly; the host's unrelated disk occupancy
// must not pause their background pipelines.
process.env.DISK_PRESSURE_MONITOR = 'false';

/** Run SQLite tests and control-plane tests without using an application database. */
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { access, mkdtemp, rm } from 'node:fs/promises';
import { createConnection, createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import pg from 'pg';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const manifest = join(root, 'apps/backend/Cargo.toml');
// Cargo filters native loader paths outside its target directory. Assign the SDK
// path in the test runner, after Cargo (and macOS shell launchers) have run.
const loaderVariable = process.platform === 'darwin' ? 'DYLD_LIBRARY_PATH' : 'LD_LIBRARY_PATH';
const analyticsFeatures = process.env.DUCKDB_LIB_DIR
  ? [
      '--no-default-features',
      '--features',
      'duckdb-analytics',
      '--config',
      `target.'cfg(all())'.runner = ${JSON.stringify([
        'env',
        `${loaderVariable}=${process.env.DUCKDB_LIB_DIR}`,
      ])}`,
    ]
  : [];
const controlTargets = [
  'control_plane',
  'ownership_fencing',
  'tenant_bootstrap',
  'tenant_replication',
  'tenant_recovery_process',
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
async function natsBinary() {
  if (process.env.NATS_SERVER_BIN) {
    await access(process.env.NATS_SERVER_BIN);
    return process.env.NATS_SERVER_BIN;
  }
  try {
    await command('nats-server', ['--version'], { capture: true });
    return 'nats-server';
  } catch {
    if (interrupted) throw new Error('Integration run interrupted');
    return undefined;
  }
}
async function localNats(bin) {
  const directory = await mkdtemp(join(tmpdir(), 'arena360-test-nats-'));
  const port = await freePort();
  const child = spawn(bin, ['-js', '-a', '127.0.0.1', '-p', String(port), '-sd', directory], {
    stdio: 'ignore',
  });
  let failure;
  child.once('error', (error) => {
    failure = error;
  });
  child.once('exit', () => {
    failure ??= new Error('Disposable NATS server exited');
  });
  const cleanup = async () => {
    if (child.exitCode === null && child.signalCode === null && child.pid) {
      const closed = new Promise((resolve) => child.once('close', resolve));
      child.kill('SIGTERM');
      const timer = setTimeout(() => child.kill('SIGKILL'), 3000);
      await closed;
      clearTimeout(timer);
    }
    await rm(directory, { recursive: true, force: true });
  };
  try {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      if (interrupted) throw new Error('Integration run interrupted');
      if (failure) throw failure;
      const ready = await new Promise((resolve) => {
        const socket = createConnection({ host: '127.0.0.1', port });
        socket.once('connect', () => {
          socket.destroy();
          resolve(true);
        });
        socket.once('error', () => {
          socket.destroy();
          resolve(false);
        });
        socket.setTimeout(100, () => {
          socket.destroy();
          resolve(false);
        });
      });
      if (ready) return { url: `nats://127.0.0.1:${port}`, cleanup };
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error('Disposable NATS server startup timed out');
  } catch (error) {
    await cleanup();
    throw error;
  }
}
async function runJetstream(env, required = false) {
  const bin = await natsBinary();
  if (bin) {
    const targets = ['outbox_publisher', 'tenant_event_stream'];
    if (process.env.DUCKDB_LIB_DIR)
      targets.push('tenant_analytics_consumer', 'tenant_analytics_rebuild', 'tenant_timezone');
    for (const target of targets) {
      const nats = await localNats(bin);
      try {
        await command(
          'cargo',
          [
            'test',
            '--manifest-path',
            manifest,
            ...analyticsFeatures,
            '--test',
            target,
            '--',
            '--ignored',
            '--test-threads=1',
          ],
          { env: { ...env, NATS_TEST_URL: nats.url, NATS_SERVER_BIN: bin } },
        );
      } finally {
        await nats.cleanup();
      }
    }
    process.stdout.write(
      '[backend integration] Disposable JetStream checks passed and servers removed.\n',
    );
  } else {
    if (required) throw new Error('Set NATS_SERVER_BIN or install nats-server');
    process.stdout.write(
      '[backend integration] JetStream gates skipped: install nats-server or set NATS_SERVER_BIN.\n',
    );
  }
  if (interrupted) throw new Error('Integration run interrupted');
}
async function main() {
  const args = process.argv.slice(2);
  if (args.length > 1 || args.some((arg) => !['--control-only', '--jetstream-only'].includes(arg)))
    throw new Error('Usage: pnpm backend:test:integration [--control-only|--jetstream-only]');
  if (args.includes('--jetstream-only'))
    return runJetstream({ ...process.env, NATS_URL: '', REDIS_URL: '' }, true);
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
      TENANT_DATA_DIR: tenantFiles,
      TMPDIR: tenantFiles,
      TMP: tenantFiles,
      TEMP: tenantFiles,
    };
    process.stdout.write(
      '[backend integration] Temporary control database created; tenant tests use isolated SQLite files.\n',
    );
    if (!args.includes('--control-only'))
      await command('cargo', ['test', '--manifest-path', manifest, ...analyticsFeatures], { env });
    // Control targets include their isolated infrastructure gates; live broker checks run separately.
    const targets = controlTargets.flatMap((target) => ['--test', target]);
    const controlNatsBinary = process.env.DUCKDB_LIB_DIR ? await natsBinary() : undefined;
    const controlNats = controlNatsBinary ? await localNats(controlNatsBinary) : undefined;
    try {
      await command(
        'cargo',
        [
          'test',
          '--manifest-path',
          manifest,
          ...analyticsFeatures,
          ...targets,
          '--',
          '--ignored',
          '--test-threads=1',
          '--show-output',
        ],
        { env: controlNats ? { ...env, NATS_TEST_URL: controlNats.url } : env },
      );
    } finally {
      await controlNats?.cleanup();
    }
    if (!args.includes('--control-only')) await runJetstream(env);
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
