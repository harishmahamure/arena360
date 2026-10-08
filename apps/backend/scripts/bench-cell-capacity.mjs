#!/usr/bin/env node
/** TEST-0050: drive the M0 harness concurrently on explicitly disposable tenants. */
import { fork } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import pg from 'pg';

const activeChildren = new Set();
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const positive = (n) => Number.isFinite(n) && n > 0;
export function validateProfile(c) {
  if (
    c.disposable !== true ||
    c.backendBuild !== 'release' ||
    !UUID.test(c.cellId) ||
    !c.hardwareProfile ||
    !c.backendRevision
  )
    throw new Error(
      'Require disposable tenants, release build, cell UUID, hardware profile and revision',
    );
  if (
    !positive(c.maxP95Ms) ||
    !positive(c.minimumMeasuredSeconds) ||
    !Number.isSafeInteger(c.repetitions) ||
    c.repetitions < 2 ||
    !Number.isSafeInteger(c.maxWaves) ||
    c.maxWaves < c.repetitions ||
    c.maxWaves > 1000
  )
    throw new Error('Require explicit latency, duration, repetitions >=2 and bounded maxWaves');
  if (
    !Array.isArray(c.tenants) ||
    !c.tenants.length ||
    new Set(c.tenants.map((t) => t.tenantId)).size !== c.tenants.length
  )
    throw new Error('Require unique benchmark tenants');
  for (const t of c.tenants)
    if (
      !UUID.test(t.tenantId) ||
      !Number.isSafeInteger(t.pcs) ||
      t.pcs < 1 ||
      t.pcs > 1000 ||
      ![t.adminUsernameEnv, t.adminPasswordEnv].every((k) => /^[A-Z][A-Z0-9_]*$/.test(k))
    )
      throw new Error('Invalid tenant UUID, PC count or credential environment names');
  if (
    !Array.isArray(c.stages) ||
    !c.stages.length ||
    c.stages.some(
      (n, i) =>
        !Number.isSafeInteger(n) ||
        n < 1 ||
        n > c.tenants.length ||
        (i > 0 && n <= c.stages[i - 1]),
    )
  )
    throw new Error('Stages must increase within the tenant population');
  if (
    !Number.isSafeInteger(c.sessions) ||
    c.sessions < 1 ||
    !Number.isSafeInteger(c.sales) ||
    c.sales < 1
  )
    throw new Error('Require positive session and sale counts');
  new URL(c.apiUrl);
  return c;
}
export function evaluateWave(c, summaries, wallMs) {
  if (!positive(wallMs) || summaries.length === 0) throw new Error('Missing measurements');
  let writes = 0,
    reads = 0,
    worstP95Ms = 0,
    failed = false;
  for (const s of summaries) {
    if (s.failedLanes || s.failure) failed = true;
    for (const [op, r] of Object.entries(s.operations ?? {})) {
      if (!Number.isSafeInteger(r.count) || r.count < 1 || !Number.isFinite(r.p95Ms) || r.p95Ms < 0)
        throw new Error('Invalid benchmark operation measurements');
      worstP95Ms = Math.max(worstP95Ms, r.p95Ms);
      if (r.failures) failed = true;
      if (op === 'balance_lookup') reads += r.count;
      else writes += r.count;
    }
    if (!Object.keys(s.operations ?? {}).length) failed = true;
  }
  return {
    passed: !failed && worstP95Ms <= c.maxP95Ms,
    worstTenantOperationP95Ms: worstP95Ms,
    writeTps: writes / (wallMs / 1000),
    readQps: reads / (wallMs / 1000),
    writes,
    reads,
    measuredSeconds: wallMs / 1000,
  };
}
async function wave(c, targets) {
  const children = [];
  try {
    const ready = targets.map(
      (t) =>
        new Promise((resolve, reject) => {
          const child = fork(
            fileURLToPath(new URL('bench-venue-day.mjs', import.meta.url)),
            ['--pcs', String(t.pcs), '--sessions', String(c.sessions), '--sales', String(c.sales)],
            {
              env: {
                ...process.env,
                BENCH_API_URL: c.apiUrl,
                BENCH_ADMIN_USERNAME: process.env[t.adminUsernameEnv],
                BENCH_ADMIN_PASSWORD: process.env[t.adminPasswordEnv],
                BENCH_TENANT_ID: t.tenantId,
                BENCH_WAIT_FOR_START: '1',
              },
              silent: true,
            },
          );
          activeChildren.add(child);
          let output = '';
          child.stdout.on('data', (b) => {
            output += b;
            if (output.length > 1024 * 1024) child.kill();
          });
          child.stderr.resume();
          const done = new Promise((resolveDone, rejectDone) => {
            child.once('error', rejectDone);
            child.once('exit', (code) => {
              try {
                const value = JSON.parse(output);
                if (code !== 0 && !value.failedLanes)
                  return rejectDone(new Error('Venue workload failed'));
                resolveDone(value);
              } catch {
                rejectDone(new Error('Invalid venue workload output'));
              }
            });
          });
          // Observe failures immediately, including during setup.
          done.catch(reject);
          children.push({ child, done });
          child.once('message', (m) =>
            m?.ready ? resolve() : reject(new Error('Invalid workload readiness')),
          );
          child.once('error', reject);
        }),
    );
    let timer;
    await Promise.race([
      Promise.all(ready),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Workload setup exceeded 120 seconds')), 120000);
      }),
    ]).finally(() => clearTimeout(timer));
    const started = performance.now();
    for (const { child } of children) child.send('start');
    let runningTimer;
    const summaries = await Promise.race([
      Promise.all(children.map((x) => x.done)),
      new Promise((_, reject) => {
        runningTimer = setTimeout(() => reject(new Error('Workload exceeded 120 seconds')), 120000);
      }),
    ]).finally(() => clearTimeout(runningTimer));
    return { summaries, ...evaluateWave(c, summaries, performance.now() - started) };
  } finally {
    for (const { child } of children) {
      activeChildren.delete(child);
      if (child.exitCode === null) child.kill();
    }
  }
}
async function ownership(pool, c, targets) {
  const result = await pool.query(
    "SELECT id FROM tenants WHERE id=ANY($1::uuid[]) AND owner_cell=$2 AND state='ACTIVE'",
    [targets.map((t) => t.tenantId), c.cellId],
  );
  if (result.rows.length !== targets.length)
    throw new Error('Benchmark tenant ownership/state differs from the selected cell');
}
export async function run(c) {
  validateProfile(c);
  for (const t of c.tenants)
    if (!process.env[t.adminUsernameEnv] || !process.env[t.adminPasswordEnv])
      throw new Error('Set every configured benchmark credential environment variable');
  if (!process.env.CONTROL_DATABASE_URL)
    throw new Error('Set CONTROL_DATABASE_URL to verify cell ownership');
  const pool = new pg.Pool({ connectionString: process.env.CONTROL_DATABASE_URL, max: 2 });
  const report = {
    version: 1,
    measuredAt: new Date().toISOString(),
    hardwareProfile: c.hardwareProfile,
    backendRevisionDeclared: c.backendRevision,
    backendBuildDeclared: c.backendBuild,
    cellId: c.cellId,
    workload: { sessions: c.sessions, sales: c.sales, standardVenuePcs: 20 },
    maxP95Ms: c.maxP95Ms,
    minimumMeasuredSeconds: c.minimumMeasuredSeconds,
    stages: [],
    observedWeightedVenues: 0,
    ceilingReached: false,
  };
  try {
    for (const count of c.stages) {
      const targets = c.tenants.slice(0, count);
      const stage = {
        tenants: count,
        weightedVenues: targets.reduce((sum, t) => sum + t.pcs / 20, 0),
        waves: [],
        passed: false,
      };
      for (let n = 0; n < c.maxWaves; n++) {
        await ownership(pool, c, targets);
        const result = await wave(c, targets);
        await ownership(pool, c, targets);
        stage.waves.push(result);
        if (!result.passed) break;
        if (
          stage.waves.length >= c.repetitions &&
          stage.waves.reduce((s, w) => s + w.measuredSeconds, 0) >= c.minimumMeasuredSeconds
        ) {
          stage.passed = true;
          break;
        }
      }
      report.stages.push(stage);
      if (!stage.passed) {
        report.ceilingReached = stage.waves.some((w) => !w.passed);
        break;
      }
      report.observedWeightedVenues = stage.weightedVenues;
    }
    return report;
  } finally {
    await pool.end();
  }
}
async function main() {
  for (const signal of ['SIGINT', 'SIGTERM'])
    process.once(signal, () => {
      for (const child of activeChildren) child.kill(signal);
      process.exit(130);
    });
  const args = process.argv.slice(2).filter((x) => x !== '--');
  if (args.includes('--help')) {
    console.log(
      'bench:cell-capacity --profile FILE --json OUTPUT\nRuns repeated synchronized M0 workloads on explicitly disposable tenants; requires CONTROL_DATABASE_URL and profile credential environment variables. No capacity extrapolation.',
    );
    return;
  }
  if (args.length !== 4 || args[0] !== '--profile' || args[2] !== '--json')
    throw new Error('Use --profile FILE --json OUTPUT');
  const report = await run(JSON.parse(await readFile(args[1], 'utf8')));
  await writeFile(args[3], `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({
      observedWeightedVenues: report.observedWeightedVenues,
      ceilingReached: report.ceilingReached,
      output: args[3],
    }),
  );
  if (report.stages.some((s) => !s.passed)) process.exitCode = 1;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((e) => {
    console.error(`Capacity benchmark failed: ${e.message}`);
    process.exitCode = 1;
  });
