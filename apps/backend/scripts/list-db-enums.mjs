#!/usr/bin/env node
/**
 * Prints Postgres enum labels from DATABASE_URL (apps/backend/.env).
 * Use when updating packages/contracts/src/enums.ts
 */
import { readFileSync } from 'node:fs';
import { Client } from 'pg';

const env = readFileSync(new URL('../.env', import.meta.url), 'utf8');
const match = env.match(/DATABASE_URL="([^"]+)"/);
if (!match) {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error('DATABASE_URL not found in apps/backend/.env');
  process.exit(1);
}

const client = new Client({ connectionString: match[1] });
await client.connect();
const types = await client.query(`
  SELECT t.typname FROM pg_type t
  JOIN pg_namespace n ON n.oid = t.typnamespace
  WHERE t.typtype = 'e' AND n.nspname = 'public'
  ORDER BY t.typname
`);
const out = {};
for (const row of types.rows) {
  const labels = await client.query(
    `SELECT enumlabel FROM pg_enum e
     JOIN pg_type ty ON e.enumtypid = ty.oid
     WHERE ty.typname = $1 ORDER BY enumsortorder`,
    [row.typname],
  );
  out[row.typname] = labels.rows.map((r) => r.enumlabel);
}
await client.end();
