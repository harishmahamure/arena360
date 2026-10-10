import assert from 'node:assert/strict';
import { test } from 'node:test';
import { evaluateWave, validateProfile } from './bench-cell-capacity.mjs';
import { salesInSession } from './bench-venue-day.mjs';

const uuid = (n) => `00000000-0000-0000-0000-${String(n).padStart(12, '0')}`;
const profile = () => ({
  disposable: true,
  backendBuild: 'release',
  cellId: uuid(1),
  hardwareProfile: 'test-fixture',
  backendRevision: 'fixture',
  maxP95Ms: 100,
  minimumMeasuredSeconds: 60,
  repetitions: 2,
  maxWaves: 10,
  sessions: 8,
  sales: 6,
  apiUrl: 'http://localhost:3000',
  stages: [1, 2],
  tenants: [2, 3].map((n) => ({
    tenantId: uuid(n),
    pcs: 20,
    adminUsernameEnv: `BENCH_USER_${n}`,
    adminPasswordEnv: `BENCH_PASSWORD_${n}`,
  })),
});
test('venue workload sells the exact requested count across uneven sessions', () => {
  for (const [sessions, sales] of [
    [8, 6],
    [8, 20],
    [1, 6],
  ])
    assert.equal(
      Array.from({ length: sessions }, (_, n) => salesInSession(n, sessions, sales)).reduce(
        (a, b) => a + b,
        0,
      ),
      sales,
    );
});
test('rejects unsafe or incomplete capacity profile before starting workloads', () => {
  assert.equal(validateProfile(profile()).stages.length, 2);
  for (const patch of [
    { disposable: false },
    { backendBuild: 'debug' },
    { maxP95Ms: NaN },
    { stages: [2, 1] },
    { repetitions: 1 },
    { tenants: [profile().tenants[0], profile().tenants[0]] },
  ])
    assert.throws(() => validateProfile({ ...profile(), ...patch }));
});
test('gates every tenant operation and separates measured read and write throughput', () => {
  const summary = {
    failedLanes: 0,
    operations: {
      session_start: { count: 20, failures: 0, p95Ms: 90 },
      balance_lookup: { count: 20, failures: 0, p95Ms: 50 },
    },
  };
  const good = evaluateWave(profile(), [summary, summary], 2000);
  assert.equal(good.passed, true);
  assert.equal(good.writeTps, 20);
  assert.equal(good.readQps, 20);
  assert.equal(evaluateWave(profile(), [{ ...summary, failedLanes: 1 }], 2000).passed, false);
  assert.equal(
    evaluateWave(
      profile(),
      [{ ...summary, operations: { session_start: { count: 20, p95Ms: 110 } } }],
      2000,
    ).passed,
    false,
  );
  assert.equal(evaluateWave(profile(), [{}], 2000).passed, false);
});
