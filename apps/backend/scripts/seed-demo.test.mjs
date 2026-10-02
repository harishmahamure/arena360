import assert from 'node:assert/strict';
import test from 'node:test';
import { Client } from 'pg';
import { buildDemo, demoId, resolveDemoDatabaseUrl, seedDemo } from './seed-demo.mjs';

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

test('seed commits once, rolls back dry runs, and emits analytics outbox events', {
  skip: !process.env.DEMO_TEST_DATABASE_URL,
}, async () => {
  const client = new Client({ connectionString: process.env.DEMO_TEST_DATABASE_URL });
  await client.connect();
  try {
    assert.equal(
      (await client.query('SELECT id FROM users WHERE id=$1', [demoId('staff')])).rowCount,
      0,
      'Use a migrated test database without demo data',
    );
    assert.equal((await seedDemo(client, { end, dryRun: true })).status, 'dry-run-rolled-back');
    assert.equal(
      (await client.query('SELECT id FROM users WHERE id=$1', [demoId('staff')])).rowCount,
      0,
    );
    const seeded = await seedDemo(client, { end });
    assert.equal(seeded.status, 'seeded');
    const before = await client.query('SELECT count(*) AS n FROM analytics_outbox');
    assert.ok(Number(before.rows[0].n) > 1000);
    assert.equal(
      (await seedDemo(client, { end: new Date('2026-10-03') })).status,
      'already-seeded',
    );
    assert.deepEqual(
      (await client.query('SELECT count(*) AS n FROM analytics_outbox')).rows,
      before.rows,
    );
    const payload = await client.query(
      'SELECT row_data FROM analytics_outbox WHERE row_id=$1 LIMIT 1',
      [demoId('staff')],
    );
    assert.equal(payload.rows[0].row_data.password_hash, undefined);
  } finally {
    await client.end();
  }
});

test('demo resolves the current backend database and supports an explicit override', () => {
  assert.equal(
    resolveDemoDatabaseUrl({ DATABASE_URL: 'postgres://current/db' }),
    'postgres://current/db',
  );
  assert.equal(
    resolveDemoDatabaseUrl({
      DEMO_DATABASE_URL: 'postgres://demo/db',
      DATABASE_URL: 'postgres://current/db',
    }),
    'postgres://demo/db',
  );
  const url = new URL(
    resolveDemoDatabaseUrl({
      DB_HOST: 'db.local',
      DB_PORT: '6432',
      DB_USERNAME: 'demo user',
      DB_PASSWORD: 'p@ss:/?#',
      DB_DATABASE: 'demo db',
    }),
  );
  assert.equal(url.hostname, 'db.local');
  assert.equal(url.port, '6432');
  assert.equal(decodeURIComponent(url.username), 'demo user');
  assert.equal(decodeURIComponent(url.password), 'p@ss:/?#');
  assert.equal(decodeURIComponent(url.pathname), '/demo db');
});
