import assert from 'node:assert/strict';
import test from 'node:test';
import { buildDemo, parseDemoArgs, resolveDemoControlDatabaseUrl, seedDemo } from './seed-demo.mjs';

const end = new Date('2026-10-02T18:00:00Z');
test('demo is deterministic, covers 60 days, and has growing activity', () => {
  const demo = buildDemo(end);
  assert.deepEqual(demo, buildDemo(end));
  assert.equal(demo.startDate, '2026-08-04');
  assert.equal(demo.endDate, '2026-10-02');
  const txs = demo.tables.transactions;
  assert.ok(txs.length > 500);
  const earlier = txs.filter((t) => t.transactionDate < '2026-09-03').length;
  assert.ok(txs.length - earlier > earlier);
  assert.deepEqual(
    new Set(txs.map((t) => t.paymentMethod)),
    new Set(['cash', 'online', 'split_payment', 'credit']),
  );
  assert.deepEqual(
    new Set(txs.map((t) => t.paymentStatus)),
    new Set(['completed', 'credit', 'pending', 'failed', 'refunded']),
  );
  assert.ok(demo.tables.usage_sessions.some((s) => s.endTime === null));
  for (const rows of Object.values(demo.tables)) {
    const ids = rows.filter((r) => r.id).map((r) => r.id);
    assert.equal(new Set(ids).size, ids.length);
    for (const row of rows) if (row.createdAt) assert.ok(new Date(row.createdAt) <= end);
  }
});

test('stock, wallet, cash, and credit ledgers reconcile with the demo balances', () => {
  const { tables: t } = buildDemo(end);
  for (const stock of t.location_stock) {
    const movements = t.stock_movements.filter(
      (m) => m.productId === stock.productId && m.locationId === stock.locationId,
    );
    assert.equal(
      movements.reduce((n, m) => n + m.delta, 0),
      stock.quantityPieces,
    );
  }
  for (const wallet of t.player_plan_balances) {
    assert.equal(
      t.player_plan_ledger
        .filter((l) => l.balanceId === wallet.id)
        .reduce((n, l) => n + l.deltaMinutes, 0),
      wallet.remainingMinutes,
    );
  }
  for (const register of t.cash_registers.filter((r) => r.status !== 'open')) {
    const entries = t.cash_register_entries.filter((e) => e.cashRegisterId === register.id);
    assert.equal(
      register.openingBalance +
        entries.reduce((n, e) => n + (e.entryType === 'cash_in' ? e.amount : -e.amount), 0),
      register.expectedClosing,
    );
    assert.equal(register.closingBalance - register.expectedClosing, register.variance);
  }
  for (const sale of t.transactions.filter((r) => r.paymentStatus === 'credit')) {
    assert.equal(
      t.credit_settlement_items
        .filter((i) => i.transactionId === sale.id)
        .reduce((n, i) => n + i.amountApplied, 0),
      sale.paidAmount,
    );
    assert.ok(sale.paidAmount < sale.amount);
  }
  for (const session of t.usage_sessions) {
    const wallet = t.player_plan_balances.find((w) => w.id === session.balanceId);
    const player = t.users.find((u) => u.id === wallet.playerId);
    assert.ok(player.createdAt <= session.startTime);
  }
});

test('seed invokes the tenant service and passes explicit ownership without operational SQL', async () => {
  let called;
  const result = await seedDemo(
    { date: '2026-10-02', tenantSlug: 'arena360-demo-test', cellId: 'cell', ownerUserId: 'owner' },
    async (args, env) => {
      called = { args, env };
      return { status: 'seeded', tenantId: 'tenant', counts: { outbox_events: 1000 } };
    },
    { CONTROL_DATABASE_URL: 'postgres://control/db', DATABASE_URL: 'postgres://old/unused' },
  );
  assert.equal(result.status, 'seeded');
  assert.deepEqual(called.args, [
    '--date',
    '2026-10-02',
    '--tenant-slug',
    'arena360-demo-test',
    '--cell-id',
    'cell',
    '--owner-user-id',
    'owner',
  ]);
  assert.equal(called.env.DEMO_CONTROL_DATABASE_URL, 'postgres://control/db');
});

test('dry-run reaches the service preview without a database environment', async () => {
  const result = await seedDemo(
    { date: '2026-10-02', dryRun: true },
    async (args, env) => {
      assert.ok(args.includes('--dry-run'));
      assert.equal(env.DEMO_CONTROL_DATABASE_URL, undefined);
      return { status: 'planned' };
    },
    {},
  );
  assert.equal(result.status, 'planned');
});

test('only explicit control database configuration is accepted', () => {
  assert.equal(
    resolveDemoControlDatabaseUrl({ CONTROL_DATABASE_URL: 'postgres://control/db' }),
    'postgres://control/db',
  );
  assert.equal(
    resolveDemoControlDatabaseUrl({
      CONTROL_DATABASE_URL: 'postgres://control/db',
      DEMO_CONTROL_DATABASE_URL: 'postgres://test/db',
    }),
    'postgres://test/db',
  );
  assert.throws(
    () => resolveDemoControlDatabaseUrl({ DATABASE_URL: 'postgres://old/db' }),
    /CONTROL_DATABASE_URL/,
  );
});

test('arguments reject invalid dates, unsafe target names and missing values', () => {
  assert.equal(parseDemoArgs(['--date', '2026-10-02', '--dry-run']).dryRun, true);
  for (const args of [
    ['--date', '2026-02-30'],
    ['--date', 'bad'],
    ['--tenant-slug', 'customer'],
    ['--cell-id'],
    ['--unknown'],
  ])
    assert.throws(() => parseDemoArgs(args));
});
