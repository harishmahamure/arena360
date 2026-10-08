#!/usr/bin/env node
import { randomInt, randomUUID } from 'node:crypto';
/**
 * Venue-day benchmark (TEST-0001). Replays a compressed day for one tenant through the
 * public HTTP API: for every PC lane, plan purchase -> session start -> session end,
 * interleaved with POS sales. Lanes run concurrently, one per PC.
 * Driving the HTTP API keeps the harness valid across the SQLite port (M3-M5) and
 * the cell capacity work (M9).
 *
 * Usage: pnpm bench:venue-day [--pcs 20] [--sessions 8] [--sales 6] [--json out.json]
 * Env: BENCH_API_URL (default http://localhost:3000), BENCH_ADMIN_USERNAME / BENCH_ADMIN_PASSWORD
 *      (default: bootstrap superadmin). The backend must run with LEGACY_REST_ENABLED=true.
 * Creates its own staff, players, PCs, plan, and product, prefixed `bench-<run>`.
 * Run it against disposable databases only.
 */
import { writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const API = process.env.BENCH_API_URL ?? 'http://localhost:3000';
const PASSWORD = 'Bench@12345';

export function parseArgs(argv) {
  const options = { pcs: 20, sessions: 8, sales: 6, json: null };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i].replace(/^--/, '');
    if (!(flag in options)) throw new Error(`Unknown argument: ${argv[i]}`);
    const value = argv[++i];
    options[flag] = flag === 'json' ? value : Number(value);
    if (flag !== 'json' && (!Number.isSafeInteger(options[flag]) || !(options[flag] > 0)))
      throw new Error(`--${flag} must be positive`);
  }
  return options;
}

const samples = new Map();
const failures = new Map();

async function call(token, method, path, body, op) {
  const started = performance.now();
  const res = await fetch(`${API}${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  if (op) {
    samples.set(op, [...(samples.get(op) ?? []), performance.now() - started]);
    if (!res.ok) failures.set(op, (failures.get(op) ?? 0) + 1);
  }
  if (!res.ok) throw new Error(`${method} ${path}: HTTP ${res.status} ${text.slice(0, 300)}`);
  return text ? JSON.parse(text).data : null;
}

const login = async (path, username, password) => {
  const token = (await call(null, 'POST', path, { username, password })).accessToken;
  if (process.env.BENCH_TENANT_ID) {
    const claims = JSON.parse(Buffer.from(token.split('.')[1], 'base64url').toString());
    if (claims.tenantId !== process.env.BENCH_TENANT_ID)
      throw new Error('Benchmark credentials selected a different tenant');
  }
  return token;
};

const phone = () => `9${String(randomInt(0, 1_000_000_000)).padStart(9, '0')}`;

async function setup(admin, run, pcs) {
  const register = (username, role, n) =>
    call(admin, 'POST', '/auth/register', {
      username,
      password: PASSWORD,
      phoneNumber: phone(n),
      firstName: 'Bench',
      lastName: run,
      role,
    });
  await register(`bench.${run}.staff`, 'staff', 0);
  const staff = await login('/auth/login/staff', `bench.${run}.staff`, PASSWORD);
  await call(staff, 'POST', '/shifts/start', { openingBalance: 0 });

  const plan = await call(admin, 'POST', '/plans', {
    name: `bench-${run} plan`,
    price: 100,
    planType: 'time_based',
    timeCredits: 600,
    validityDays: 30,
    deviceType: 'PC',
    deviceSubType: 'HIGH_END_PCS',
  });
  const product = await call(admin, 'POST', '/products', {
    name: `bench-${run} cola`,
    price: 50,
    purchasePrice: 20,
    category: 'beverage',
  });
  const store = await call(admin, 'POST', '/inventory/locations', {
    name: `bench-${run} counter`,
    kind: 'store',
  });
  await call(admin, 'POST', '/inventory/adjustments', {
    locationId: store.id,
    notes: `bench-${run} opening stock`,
    lines: [{ productId: product.id, countedPieces: 100_000 }],
  });
  const lanes = [];
  for (let i = 0; i < pcs; i++) {
    const device = await call(admin, 'POST', '/devices', {
      name: `bench-${run} PC ${i + 1}`,
      deviceType: 'PC',
      deviceSubType: 'HIGH_END_PCS',
      status: 'available',
    });
    const username = `bench.${run}.p${i + 1}`;
    await register(username, 'player', i + 1);
    const users = await call(
      admin,
      'GET',
      `/users?${new URLSearchParams({ username, limit: '1' })}`,
    );
    lanes.push({ deviceId: device.id, playerId: (users.data ?? users)[0].id });
  }
  return { staff, planId: plan.id, productId: product.id, storeId: store.id, lanes };
}

export const salesInSession = (session, sessions, sales) =>
  Math.floor(((session + 1) * sales) / sessions) - Math.floor((session * sales) / sessions);

async function runLane(ctx, lane, sessions, sales) {
  const { staff, planId, productId, storeId } = ctx;
  for (let s = 0; s < sessions; s++) {
    await call(
      staff,
      'POST',
      '/transactions',
      {
        playerId: lane.playerId,
        transactionType: 'plan_purchase',
        planId,
        paymentMethod: 'cash',
        paymentStatus: 'completed',
      },
      'plan_purchase',
    );
    const balances = await call(
      staff,
      'GET',
      `/player-plans?${new URLSearchParams({ playerId: lane.playerId, status: 'active', sortBy: 'createdAt', sortOrder: 'DESC', limit: '1' })}`,
      null,
      'balance_lookup',
    );
    const balanceId = (balances.data ?? balances)[0].id;
    const session = await call(
      staff,
      'POST',
      '/sessions',
      { balanceId, deviceId: lane.deviceId },
      'session_start',
    );
    for (let k = 0; k < salesInSession(s, sessions, sales); k++) {
      await call(
        staff,
        'POST',
        '/transactions',
        {
          playerId: lane.playerId,
          transactionType: 'product_purchase',
          paymentMethod: 'cash',
          paymentStatus: 'completed',
          saleLocationId: storeId,
          lineItems: [{ productId, quantity: 1 }],
        },
        'pos_sale',
      );
    }
    await call(
      staff,
      'PATCH',
      `/sessions/${session.id}/end`,
      { reason: 'voluntary' },
      'session_end',
    );
  }
}

const percentile = (sorted, p) =>
  sorted[Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1)];

function summarize(wallMs) {
  const operations = {};
  let total = 0;
  for (const [op, values] of samples) {
    const sorted = [...values].sort((a, b) => a - b);
    total += sorted.length;
    operations[op] = {
      count: sorted.length,
      failures: failures.get(op) ?? 0,
      p50Ms: +percentile(sorted, 50).toFixed(1),
      p95Ms: +percentile(sorted, 95).toFixed(1),
      p99Ms: +percentile(sorted, 99).toFixed(1),
      maxMs: +sorted[sorted.length - 1].toFixed(1),
    };
  }
  return {
    wallSeconds: +(wallMs / 1000).toFixed(2),
    operationsPerSecond: +(total / (wallMs / 1000)).toFixed(1),
    operations,
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2).filter((a) => a !== '--'));
  const run = randomUUID().slice(0, 13).replaceAll('-', '');
  const admin = await login(
    '/auth/login/admin',
    process.env.BENCH_ADMIN_USERNAME ?? 'superadmin',
    process.env.BENCH_ADMIN_PASSWORD ?? 'SuperAdmin@123',
  );
  const ctx = await setup(admin, run, options.pcs);
  if (process.env.BENCH_WAIT_FOR_START === '1') {
    if (!process.send) throw new Error('Capacity benchmark requires an IPC parent');
    process.send({ ready: true });
    await new Promise((resolve, reject) =>
      process.once('message', (message) =>
        message === 'start' ? resolve() : reject(new Error('Invalid capacity start signal')),
      ),
    );
  }
  const started = performance.now();
  const results = await Promise.allSettled(
    ctx.lanes.map((lane) => runLane(ctx, lane, options.sessions, options.sales)),
  );
  const summary = {
    run,
    ...options,
    failedLanes: results.filter((r) => r.status === 'rejected').length,
    ...summarize(performance.now() - started),
  };
  const firstError = results.find((r) => r.status === 'rejected')?.reason?.message;
  if (firstError) summary.firstError = firstError;
  const text = `${JSON.stringify(summary, null, 2)}\n`;
  if (options.json) await writeFile(options.json, text);
  process.stdout.write(text);
  if (process.connected) process.disconnect();
  if (summary.failedLanes) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error) => {
    process.stderr.write(`Benchmark failed: ${error.message}\n`);
    process.exitCode = 1;
  });
