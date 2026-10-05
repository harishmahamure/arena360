#!/usr/bin/env node
/**
 * Captures golden report outputs (OPS-0001) from a running backend that holds only the
 * demo dataset (`pnpm demo:seed --date 2026-10-02`) with analytics fully ingested.
 * Fixtures are the parity baseline for the DuckDB rewrite (docs/plans/data-platform-build-plan.md, M7).
 * The backend must run with LEGACY_REST_ENABLED=true; the RPC gateway serves the same router.
 *
 * Usage: pnpm report:fixtures [--check]
 *   --check  compare against the committed fixtures instead of writing them.
 * Env: FIXTURE_API_URL (default http://localhost:3000),
 *      FIXTURE_ADMIN_USERNAME / FIXTURE_ADMIN_PASSWORD (default: bootstrap superadmin).
 */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { isDeepStrictEqual } from 'node:util';

const API = process.env.FIXTURE_API_URL ?? 'http://localhost:3000';
const OUT = new URL('../tests/fixtures/reports/', import.meta.url);
const READY_TIMEOUT_MS = 120_000;

const MONTH = { startDate: '2026-09-03', endDate: '2026-10-02' };
const WEEK = { startDate: '2026-09-26', endDate: '2026-10-02' };
const DAY = { startDate: '2026-10-02', endDate: '2026-10-02' };
const FULL_RANGE = { from: '2026-08-04T00:00:00Z', to: '2026-10-03T00:00:00Z' };

const STATS = [
  'dashboard',
  'staff-dashboard',
  'revenue/by-payment-method',
  'usage',
  'finance/reconciliation',
  'finance/deposits',
  'finance/variance',
  'business',
];

const CASES = [
  ...STATS.flatMap((report) =>
    Object.entries({ month: MONTH, week: WEEK, day: DAY }).map(([window, query]) => ({
      name: `stats-${report.replaceAll('/', '-')}.${window}`,
      path: `/stats/${report}`,
      query,
    })),
  ),
  { name: 'stats-finance-report.month', path: '/stats/finance/report', query: MONTH },
  { name: 'stats-finance-report.day', path: '/stats/finance/report', query: DAY },
  { name: 'expenses-summary', path: '/expenses/summary' },
  { name: 'credit-summary', path: '/credit/summary' },
  { name: 'inventory-overview', path: '/inventory/overview' },
  { name: 'inventory-receipts-summary', path: '/inventory/receipts/summary', query: FULL_RANGE },
  { name: 'inventory-waste-summary', path: '/inventory/waste/summary', query: FULL_RANGE },
];

/** Response fields that record when the report ran rather than what it contains. */
const VOLATILE_KEYS = new Set(['generatedAt']);
/** Lists ordered only by `createdAt DESC`, so rows sharing a timestamp come back in any order. */
const TIE_ORDERED_LISTS = new Set(['recentMovements']);

const request = (path, query) => (query ? `${path}?${new URLSearchParams(query)}` : path);

const byCreatedAtThenId = (a, b) =>
  b.createdAt.localeCompare(a.createdAt) || a.id.localeCompare(b.id);

const normalize = (value) =>
  Array.isArray(value)
    ? value.map(normalize)
    : value && typeof value === 'object'
      ? Object.fromEntries(
          Object.entries(value)
            .filter(([key]) => !VOLATILE_KEYS.has(key))
            .map(([key, v]) => {
              const normalized = normalize(v);
              return [
                key,
                TIE_ORDERED_LISTS.has(key) ? [...normalized].sort(byCreatedAtThenId) : normalized,
              ];
            }),
        )
      : value;

function firstDifference(a, b, path = '$') {
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
      const found = firstDifference(a[key], b[key], `${path}.${key}`);
      if (found) return found;
    }
    return null;
  }
  return a === b ? null : `${path}: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`;
}

async function login() {
  const res = await fetch(`${API}/auth/login/admin`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      username: process.env.FIXTURE_ADMIN_USERNAME ?? 'superadmin',
      password: process.env.FIXTURE_ADMIN_PASSWORD ?? 'SuperAdmin@123',
    }),
  });
  const body = await res.json();
  const token = body?.data?.accessToken;
  if (!res.ok || !token) throw new Error(`Admin login failed with HTTP ${res.status}`);
  return token;
}

async function fetchReport(token, target) {
  const deadline = Date.now() + READY_TIMEOUT_MS;
  while (true) {
    const res = await fetch(`${API}${target}`, { headers: { authorization: `Bearer ${token}` } });
    const text = await res.text();
    if (res.ok) return normalize(JSON.parse(text).data);
    // Analytics answers 503 until ingestion is ready.
    if (res.status !== 503 || Date.now() > deadline)
      throw new Error(`${target}: HTTP ${res.status} ${text.slice(0, 300)}`);
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  }
}

async function captureAll(token) {
  const results = new Map();
  for (const { name, path, query } of CASES) {
    const target = request(path, query);
    results.set(name, { request: target, data: await fetchReport(token, target) });
  }
  return results;
}

async function main() {
  const check = process.argv.includes('--check');
  const token = await login();
  const first = await captureAll(token);
  const second = await captureAll(token);
  const unstable = [...first.keys()]
    .filter((name) => !isDeepStrictEqual(first.get(name), second.get(name)))
    .map((name) => `${name} (${firstDifference(first.get(name), second.get(name))})`);
  if (unstable.length) throw new Error(`Non-deterministic reports: ${unstable.join('; ')}`);

  const mismatched = [];
  if (!check) await mkdir(OUT, { recursive: true });
  for (const [name, fixture] of first) {
    const file = new URL(`${name}.json`, OUT);
    const text = `${JSON.stringify(fixture, null, 2)}\n`;
    if (!check) await writeFile(file, text);
    else if ((await readFile(file, 'utf8').catch(() => '')) !== text) mismatched.push(name);
  }
  if (mismatched.length) throw new Error(`Fixtures differ: ${mismatched.join(', ')}`);
  process.stdout.write(`${check ? 'Verified' : 'Wrote'} ${first.size} report fixtures\n`);
}

main().catch((error) => {
  process.stderr.write(`Report fixture capture failed: ${error.message}\n`);
  process.exitCode = 1;
});
