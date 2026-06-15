#!/usr/bin/env node
/**
 * Import legacy player plan CSV rows into player_plan_balances (+ ledger).
 *
 * Usage:
 *   node apps/backend/scripts/import-plan-balances.mjs \
 *     --plans-csv plans.csv \
 *     [--users-csv users.csv] \
 *     [--database-url "$DATABASE_URL"] \
 *     [--created-by <admin-uuid>] \
 *     [--seed-plans] \
 *     [--dry-run]
 *
 * DATABASE_URL is read from --database-url, process.env.DATABASE_URL, or apps/backend/.env.
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Client } from 'pg';

const BALANCE_STATUSES = new Set(['active', 'expired', 'exhausted', 'cancelled']);

function usage() {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error(`Usage:
  node apps/backend/scripts/import-plan-balances.mjs \\
    --plans-csv <path> \\
    [--users-csv <path>] \\
    [--database-url <postgres-url>] \\
    [--created-by <admin-uuid>] \\
    [--seed-plans] \\
    [--dry-run]

Options:
  --plans-csv       Required. Legacy export with username, planType, price, name,
                    deviceSubType, deviceType, purchaseDate, expiryDate,
                    remainingTimeCredits, status. Plan IDs are resolved from DB at import time.
  --users-csv       Optional. Upsert players by username first; player IDs are always read back from DB.
  --database-url    Postgres connection string (else DATABASE_URL or apps/backend/.env).
  --created-by      Admin user id for createdBy/updatedBy (default: first admin/superadmin).
  --seed-plans      Insert missing catalog plans from unique CSV rows before import.
  --dry-run         Parse and validate only; no writes.
`);
}

function parseArgs(argv) {
  const args = {
    plansCsv: null,
    usersCsv: null,
    databaseUrl: null,
    createdBy: null,
    seedPlans: false,
    dryRun: false,
  };

  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--') {
      continue;
    }
    switch (token) {
      case '--plans-csv':
        args.plansCsv = argv[++i];
        break;
      case '--users-csv':
        args.usersCsv = argv[++i];
        break;
      case '--database-url':
        args.databaseUrl = argv[++i];
        break;
      case '--created-by':
        args.createdBy = argv[++i];
        break;
      case '--seed-plans':
        args.seedPlans = true;
        break;
      case '--dry-run':
        args.dryRun = true;
        break;
      case '-h':
      case '--help':
        usage();
        process.exit(0);
        break;
      default:
        // biome-ignore lint/suspicious/noConsole: CLI script
        console.error(`Unknown argument: ${token}`);
        usage();
        process.exit(1);
    }
  }

  if (!args.plansCsv) {
    usage();
    process.exit(1);
  }

  return args;
}

function loadDatabaseUrl(cliUrl) {
  if (cliUrl) return cliUrl;
  if (process.env.DATABASE_URL) return process.env.DATABASE_URL;

  const envPath = new URL('../.env', import.meta.url);
  const env = readFileSync(envPath, 'utf8');

  const urlMatch = env.match(/DATABASE_URL=(?:"([^"]+)"|([^\s#]+))/);
  if (urlMatch) {
    const url = urlMatch[1] ?? urlMatch[2];
    if (url) return url;
  }

  const pick = (key) => {
    const match = env.match(new RegExp(`${key}=(.+)`));
    return match?.[1]?.trim();
  };
  const host = pick('DB_HOST');
  const port = pick('DB_PORT');
  const username = pick('DB_USERNAME');
  const password = pick('DB_PASSWORD');
  const database = pick('DB_DATABASE');
  if (host && port && username && password && database) {
    return `postgres://${encodeURIComponent(username)}:${encodeURIComponent(password)}@${host}:${port}/${database}`;
  }

  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error(
    'DATABASE_URL not found. Pass --database-url or set DATABASE_URL / DB_* in apps/backend/.env',
  );
  process.exit(1);
}

/** Minimal RFC-style CSV parser (quoted fields, commas). */
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    const next = text[i + 1];

    if (inQuotes) {
      if (ch === '"' && next === '"') {
        field += '"';
        i += 1;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        field += ch;
      }
      continue;
    }

    if (ch === '"') {
      inQuotes = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
    } else if (ch === '\n') {
      row.push(field);
      if (row.some((cell) => cell.length > 0)) rows.push(row);
      row = [];
      field = '';
    } else if (ch === '\r') {
      // skip
    } else {
      field += ch;
    }
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    if (row.some((cell) => cell.length > 0)) rows.push(row);
  }

  return rows;
}

function rowsToObjects(rows) {
  if (rows.length === 0) return [];
  const headers = rows[0].map((h) => h.trim());
  return rows.slice(1).map((cells) => {
    const obj = {};
    for (let i = 0; i < headers.length; i += 1) {
      obj[headers[i]] = cells[i] ?? '';
    }
    return obj;
  });
}

function readCsvObjects(path) {
  const absolute = resolve(path);
  const text = readFileSync(absolute, 'utf8');
  return rowsToObjects(parseCsv(text));
}

function normalizeUsername(username) {
  return String(username ?? '')
    .trim()
    .replace(/\s+/g, '_');
}

function usernameLookupKey(username) {
  return normalizeUsername(username).toLowerCase();
}

function emptyToNull(value) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function normalizePrice(value) {
  const num = Number.parseFloat(String(value ?? '').trim());
  if (Number.isNaN(num)) return null;
  return num.toFixed(2);
}

function normalizePlanName(name) {
  return name.trim().replace(/\s+/g, ' ').toLowerCase();
}

function inferDeviceTypeFromName(name) {
  if (/ps\s*5|ps5/i.test(name)) return 'PS5';
  if (/ps\s*4|ps4/i.test(name)) return 'PS4';
  return null;
}

function parseTimeCreditsFromName(name) {
  const lower = name.toLowerCase();
  const hourMatch = lower.match(/(\d+(?:\.\d+)?)\s*hours?/);
  if (hourMatch) return Math.round(Number.parseFloat(hourMatch[1]) * 60);
  const minMatch = lower.match(/(\d+)\s*min/);
  if (minMatch) return Number.parseInt(minMatch[1], 10);
  return null;
}

function parseValidityDaysFromName(name) {
  const match = name.match(/(\d+)\s*[dD]\b/);
  if (match) return Number.parseInt(match[1], 10);
  if (/1\s*day/i.test(name)) return 1;
  return 7;
}

function csvPlanScope(row) {
  const name = row.name?.trim() ?? '';
  let deviceType = emptyToNull(row.deviceType);
  let deviceSubType = emptyToNull(row.deviceSubType);
  if (!deviceType) {
    deviceType = inferDeviceTypeFromName(name);
  }
  if (deviceType === 'PS5' && !deviceSubType) {
    deviceSubType = 'PREMIUM_TV_CONSOLES';
  }
  return {
    planType: emptyToNull(row.planType) ?? 'time_based',
    price: normalizePrice(row.price),
    name: name.trim(),
    deviceType,
    deviceSubType,
  };
}

function planMatchKey(scope, { nameOnly = false, deviceOnly = false } = {}) {
  if (nameOnly) {
    return `${scope.planType}|${scope.price}|${normalizePlanName(scope.name)}`;
  }
  if (deviceOnly) {
    return `${scope.planType}|${scope.price}|${scope.deviceType ?? ''}|${scope.deviceSubType ?? ''}`;
  }
  return `${scope.planType}|${scope.price}|${normalizePlanName(scope.name)}|${scope.deviceType ?? ''}|${scope.deviceSubType ?? ''}`;
}

function deviceScopeKey(scope) {
  return `${scope.planType}|${scope.deviceType ?? ''}|${scope.deviceSubType ?? ''}`;
}

function pickPlanByDeviceScope(scope, byDeviceScope) {
  const candidates = byDeviceScope.get(deviceScopeKey(scope));
  if (!candidates?.length) return null;

  const targetPrice = Number.parseFloat(scope.price);
  if (Number.isNaN(targetPrice)) return candidates[0];

  let best = candidates[0];
  let bestDiff = Math.abs(best.price - targetPrice);
  for (const plan of candidates) {
    const diff = Math.abs(plan.price - targetPrice);
    if (diff < bestDiff) {
      best = plan;
      bestDiff = diff;
    }
  }
  return best;
}

function buildPlanIndexes(plans) {
  const byId = new Map();
  const byFullKey = new Map();
  const byNameKey = new Map();
  const byDeviceKey = new Map();
  const byDeviceScope = new Map();

  for (const plan of plans) {
    byId.set(plan.id, plan);
    const scope = {
      planType: plan.plan_type,
      price: normalizePrice(plan.price),
      name: plan.name,
      deviceType: plan.device_type,
      deviceSubType: plan.device_sub_type,
    };
    byFullKey.set(planMatchKey(scope), plan);
    byNameKey.set(planMatchKey(scope, { nameOnly: true }), plan);
    byDeviceKey.set(planMatchKey(scope, { deviceOnly: true }), plan);

    const scopeKey = deviceScopeKey(scope);
    if (!byDeviceScope.has(scopeKey)) {
      byDeviceScope.set(scopeKey, []);
    }
    byDeviceScope.get(scopeKey).push(plan);
  }

  for (const list of byDeviceScope.values()) {
    list.sort((a, b) => a.price - b.price);
  }

  return { byId, byFullKey, byNameKey, byDeviceKey, byDeviceScope };
}

function resolvePlanForCsvRow(row, indexes) {
  const scope = csvPlanScope(row);
  if (!scope.name || !scope.price) {
    return { plan: null, planId: null, match: null, scope };
  }

  let plan = indexes.byFullKey.get(planMatchKey(scope));
  if (plan) return { plan, planId: plan.id, match: 'full', scope };

  plan = indexes.byNameKey.get(planMatchKey(scope, { nameOnly: true }));
  if (plan) return { plan, planId: plan.id, match: 'name', scope };

  plan = indexes.byDeviceKey.get(planMatchKey(scope, { deviceOnly: true }));
  if (plan) return { plan, planId: plan.id, match: 'device', scope };

  if (scope.deviceType || scope.deviceSubType) {
    plan = pickPlanByDeviceScope(scope, indexes.byDeviceScope);
    if (plan) return { plan, planId: plan.id, match: 'deviceScope', scope };
  }

  return { plan: null, planId: null, match: null, scope };
}

function uniqueCatalogRows(planRows) {
  const map = new Map();
  for (const row of planRows) {
    const scope = csvPlanScope(row);
    if (!scope.name || !scope.price) continue;
    const key = planMatchKey(scope);
    if (!map.has(key)) {
      map.set(key, { scope, sampleRow: row });
    }
  }
  return [...map.values()];
}

async function loadPlans(client) {
  const result = await client.query(
    `SELECT id, name, price::float8 AS price, "planType"::text AS plan_type,
            "deviceType"::text AS device_type,
            "deviceSubType"::text AS device_sub_type,
            "timeWindowStart" AS window_start,
            "timeWindowEnd" AS window_end,
            "allowedDays" AS allowed_days,
            "allowedMonths" AS allowed_months,
            "deductionProfile" AS deduction_profile,
            "dynamicDeductionEnabled" AS dynamic_deduction_enabled
     FROM plans
     WHERE "deletedAt" IS NULL`,
  );
  return result.rows;
}

async function seedMissingPlans(client, planRows, adminId, dryRun) {
  const plans = await loadPlans(client);
  const indexes = buildPlanIndexes(plans);
  const catalogRows = uniqueCatalogRows(planRows);
  let inserted = 0;
  let skipped = 0;

  for (const { scope, sampleRow } of catalogRows) {
    const resolved = resolvePlanForCsvRow(sampleRow, indexes);
    if (resolved.plan) {
      skipped += 1;
      continue;
    }

    const parsedCredits = parseTimeCreditsFromName(scope.name);
    const fallbackCredits = Number.parseInt(sampleRow.remainingTimeCredits, 10);
    const timeCredits = parsedCredits ?? (Number.isNaN(fallbackCredits) ? 60 : fallbackCredits);
    const validityDays = parseValidityDaysFromName(scope.name);

    if (dryRun) {
      const fakePlan = {
        id: `dry-run:${planMatchKey(scope)}`,
        name: scope.name,
        price: Number.parseFloat(scope.price),
        plan_type: scope.planType,
        device_type: scope.deviceType,
        device_sub_type: scope.deviceSubType,
        window_start: null,
        window_end: null,
        allowed_days: null,
        allowed_months: null,
        deduction_profile: null,
        dynamic_deduction_enabled: false,
      };
      indexes.byId.set(fakePlan.id, fakePlan);
      indexes.byFullKey.set(planMatchKey(scope), fakePlan);
      indexes.byNameKey.set(planMatchKey(scope, { nameOnly: true }), fakePlan);
      indexes.byDeviceKey.set(planMatchKey(scope, { deviceOnly: true }), fakePlan);
      const fakeScopeKey = deviceScopeKey(scope);
      if (!indexes.byDeviceScope.has(fakeScopeKey)) {
        indexes.byDeviceScope.set(fakeScopeKey, []);
      }
      indexes.byDeviceScope.get(fakeScopeKey).push(fakePlan);
      indexes.byDeviceScope.get(fakeScopeKey).sort((a, b) => a.price - b.price);
      inserted += 1;
      continue;
    }

    const insertedPlan = await client.query(
      `INSERT INTO plans (
         id, name, description, price, "planType", "validityDays",
         "timeWindowStart", "timeWindowEnd", "timeCredits",
         "isActive", "deviceType", "deviceSubType",
         "allowedDays", "allowedMonths",
         "dynamicDeductionEnabled", "deductionProfile",
         "createdBy", "updatedBy", "createdAt", "updatedAt"
       )
       VALUES (
         gen_random_uuid(), $1, NULL, $2::numeric, $3::plans_plantype_enum, $4,
         NULL, NULL, $5,
         true, $6::plans_devicetype_enum, $7::plans_devicesubtype_enum,
         NULL, NULL,
         false, NULL,
         $8, $8, NOW(), NOW()
       )
       RETURNING id, name, price::float8 AS price, "planType"::text AS plan_type,
                 "deviceType"::text AS device_type,
                 "deviceSubType"::text AS device_sub_type,
                 "timeWindowStart" AS window_start,
                 "timeWindowEnd" AS window_end,
                 "allowedDays" AS allowed_days,
                 "allowedMonths" AS allowed_months,
                 "deductionProfile" AS deduction_profile,
                 "dynamicDeductionEnabled" AS dynamic_deduction_enabled`,
      [
        scope.name,
        scope.price,
        scope.planType,
        validityDays,
        timeCredits,
        scope.deviceType,
        scope.deviceSubType,
        adminId,
      ],
    );

    const plan = insertedPlan.rows[0];
    indexes.byId.set(plan.id, plan);
    indexes.byFullKey.set(planMatchKey(scope), plan);
    indexes.byNameKey.set(planMatchKey(scope, { nameOnly: true }), plan);
    indexes.byDeviceKey.set(planMatchKey(scope, { deviceOnly: true }), plan);
    const scopeKey = deviceScopeKey(scope);
    if (!indexes.byDeviceScope.has(scopeKey)) {
      indexes.byDeviceScope.set(scopeKey, []);
    }
    indexes.byDeviceScope.get(scopeKey).push(plan);
    indexes.byDeviceScope.get(scopeKey).sort((a, b) => a.price - b.price);
    inserted += 1;
  }

  return { inserted, skipped, indexes };
}

function mapBalanceStatus(raw) {
  const status = raw.trim().toLowerCase();
  if (BALANCE_STATUSES.has(status)) return status;
  if (status === 'moved_to_next_plan') return 'cancelled';
  return 'active';
}

function planKind(planType) {
  return planType === 'weekend_special' ? 'happy_hours' : 'time';
}

async function resolveAdminId(client, createdByArg) {
  if (createdByArg) {
    const check = await client.query(
      `SELECT id FROM users WHERE id = $1 AND role IN ('admin', 'superadmin') AND "deletedAt" IS NULL`,
      [createdByArg],
    );
    if (check.rowCount === 0) {
      throw new Error(`--created-by ${createdByArg} is not an active admin user`);
    }
    return createdByArg;
  }

  const result = await client.query(
    `SELECT id FROM users
     WHERE role IN ('admin', 'superadmin') AND "deletedAt" IS NULL
     ORDER BY CASE WHEN username = 'superadmin' THEN 0 ELSE 1 END, "createdAt" ASC
     LIMIT 1`,
  );
  if (result.rowCount === 0) {
    throw new Error('No admin user found. Seed superadmin or pass --created-by.');
  }
  return result.rows[0].id;
}

async function loadDbUsernameIndex(client) {
  const result = await client.query(`SELECT id, username FROM users WHERE "deletedAt" IS NULL`);
  const map = new Map();
  for (const row of result.rows) {
    map.set(usernameLookupKey(row.username), row.id);
  }
  return map;
}

async function upsertUsers(client, users, adminId, dryRun) {
  let inserted = 0;
  let updated = 0;
  let skipped = 0;
  let normalized = 0;
  let duplicateUsernames = 0;
  const seenUsernames = new Set();

  for (const user of users) {
    const rawUsername = user.username ?? '';
    const username = normalizeUsername(rawUsername);
    if (!username) {
      skipped += 1;
      continue;
    }
    if (seenUsernames.has(username)) {
      duplicateUsernames += 1;
      skipped += 1;
      continue;
    }
    seenUsernames.add(username);
    if (username !== rawUsername.trim()) {
      normalized += 1;
    }

    const email = user.email?.trim() || null;
    const passwordHash = user.password_hash?.trim();
    if (!passwordHash) {
      skipped += 1;
      continue;
    }

    const firstName = user.firstName?.trim() || null;
    const lastName = user.lastName?.trim() || null;
    const role = user.role?.trim() || 'player';
    const isActive = String(user.isActive ?? 'true').toLowerCase() !== 'false';
    const phoneNumber = user.phoneNumber?.trim() || '0000000000';
    const createdAt = user.createdAt?.trim() || null;
    const updatedAt = user.updatedAt?.trim() || null;

    if (dryRun) {
      const existing = await client.query(
        `SELECT id FROM users WHERE username = $1 AND "deletedAt" IS NULL`,
        [username],
      );
      if (existing.rowCount > 0) updated += 1;
      else inserted += 1;
      continue;
    }

    const result = await client.query(
      `INSERT INTO users (
         id, email, username, password_hash, "firstName", "lastName",
         role, "isActive", "phoneNumber", "createdBy", "updatedBy",
         "createdAt", "updatedAt"
       )
       VALUES (
         gen_random_uuid(), $1, $2, $3, $4, $5,
         $6, $7, $8, $9, $9,
         COALESCE($10::timestamptz, NOW()), COALESCE($11::timestamptz, NOW())
       )
       ON CONFLICT (username) DO UPDATE SET
         email = EXCLUDED.email,
         password_hash = EXCLUDED.password_hash,
         "firstName" = EXCLUDED."firstName",
         "lastName" = EXCLUDED."lastName",
         role = EXCLUDED.role,
         "isActive" = EXCLUDED."isActive",
         "phoneNumber" = EXCLUDED."phoneNumber",
         "updatedBy" = EXCLUDED."updatedBy",
         "updatedAt" = NOW()
       RETURNING (xmax = 0) AS inserted`,
      [
        email,
        username,
        passwordHash,
        firstName,
        lastName,
        role,
        isActive,
        phoneNumber,
        adminId,
        createdAt,
        updatedAt,
      ],
    );

    if (result.rows[0]?.inserted) inserted += 1;
    else updated += 1;
  }

  return { inserted, updated, skipped, normalized, duplicateUsernames };
}

async function findBalanceForScope(client, playerId, planRow) {
  const kind = planKind(planRow.plan_type);
  const result = await client.query(
    `SELECT id, status::text AS status
     FROM player_plan_balances
     WHERE "playerId" = $1
       AND COALESCE("deviceType"::text, '__null__') = COALESCE($2, '__null__')
       AND COALESCE("deviceSubType"::text, '__null__') = COALESCE($3, '__null__')
       AND kind::text = $4
       AND "deletedAt" IS NULL
     ORDER BY "createdAt" DESC
     LIMIT 1`,
    [playerId, planRow.device_type, planRow.device_sub_type, kind],
  );
  return result.rows[0] ?? null;
}

async function demoteConflictingActive(client, playerId, planRow, kind, excludeId, adminId) {
  await client.query(
    `UPDATE player_plan_balances
     SET status = 'cancelled'::balance_status,
         "updatedBy" = $5,
         "updatedAt" = NOW()
     WHERE "playerId" = $1
       AND COALESCE("deviceType"::text, '__null__') = COALESCE($2, '__null__')
       AND COALESCE("deviceSubType"::text, '__null__') = COALESCE($3, '__null__')
       AND kind::text = $4
       AND status::text = 'active'
       AND "deletedAt" IS NULL
       AND ($6::uuid IS NULL OR id <> $6)`,
    [playerId, planRow.device_type, planRow.device_sub_type, kind, adminId, excludeId],
  );
}

async function upsertBalance(client, row, playerId, planRow, adminId, dryRun, planId) {
  const remainingMinutes = Number.parseInt(row.remainingTimeCredits, 10);
  if (Number.isNaN(remainingMinutes)) {
    throw new Error(`Invalid remainingTimeCredits for ${row.username}`);
  }

  const expiryDate = row.expiryDate?.trim();
  if (!expiryDate) {
    throw new Error(`Missing expiryDate for ${row.username}`);
  }

  const status = mapBalanceStatus(row.status ?? 'active');
  const kind = planKind(planRow.plan_type);
  const deductionProfile =
    planRow.dynamic_deduction_enabled && planRow.deduction_profile
      ? planRow.deduction_profile
      : null;

  if (dryRun) {
    return { action: 'dry-run', balanceId: null };
  }

  const existing = await findBalanceForScope(client, playerId, planRow);

  if (status === 'active') {
    await demoteConflictingActive(client, playerId, planRow, kind, existing?.id ?? null, adminId);
  }

  let balanceId;
  if (existing) {
    const updated = await client.query(
      `UPDATE player_plan_balances SET
         "remainingMinutes" = $2,
         "expiryDate" = $3::timestamptz,
         "windowStart" = $4,
         "windowEnd" = $5,
         status = $6::balance_status,
         "sourcePlanId" = $7,
         "allowedDays" = $8,
         "allowedMonths" = $9,
         "deductionProfile" = COALESCE($10, "deductionProfile"),
         "updatedBy" = $11,
         "updatedAt" = NOW()
       WHERE id = $1
       RETURNING id`,
      [
        existing.id,
        remainingMinutes,
        expiryDate,
        planRow.window_start,
        planRow.window_end,
        status,
        planId,
        planRow.allowed_days,
        planRow.allowed_months,
        deductionProfile,
        adminId,
      ],
    );
    balanceId = updated.rows[0].id;
    await client.query(
      `INSERT INTO player_plan_ledger (
         "balanceId", "playerId", "deltaMinutes", reason,
         "balanceAfter", "expiryAfter", "createdBy"
       )
       VALUES ($1, $2, $3, 'migration'::ledger_reason, $3, $4::timestamptz, $5)`,
      [balanceId, playerId, remainingMinutes, expiryDate, adminId],
    );
    return { action: 'updated', balanceId };
  }

  const inserted = await client.query(
    `INSERT INTO player_plan_balances (
       "playerId", "deviceType", "deviceSubType", kind,
       "remainingMinutes", "expiryDate", "windowStart", "windowEnd",
       status, "sourcePlanId", "allowedDays", "allowedMonths",
       "deductionProfile", "createdBy", "updatedBy"
     )
     VALUES (
       $1, $2::plans_devicetype_enum, $3::plans_devicesubtype_enum, $4::plan_kind,
       $5, $6::timestamptz, $7, $8,
       $9::balance_status, $10, $11, $12,
       $13, $14, $14
     )
     RETURNING id`,
    [
      playerId,
      planRow.device_type,
      planRow.device_sub_type,
      kind,
      remainingMinutes,
      expiryDate,
      planRow.window_start,
      planRow.window_end,
      status,
      planId,
      planRow.allowed_days,
      planRow.allowed_months,
      deductionProfile,
      adminId,
    ],
  );
  balanceId = inserted.rows[0].id;
  await client.query(
    `INSERT INTO player_plan_ledger (
       "balanceId", "playerId", "deltaMinutes", reason,
       "balanceAfter", "expiryAfter", "createdBy"
     )
     VALUES ($1, $2, $3, 'migration'::ledger_reason, $3, $4::timestamptz, $5)`,
    [balanceId, playerId, remainingMinutes, expiryDate, adminId],
  );
  return { action: 'inserted', balanceId };
}

async function main() {
  const args = parseArgs(process.argv);
  const databaseUrl = loadDatabaseUrl(args.databaseUrl);

  const planRows = readCsvObjects(args.plansCsv);
  const userRows = args.usersCsv ? readCsvObjects(args.usersCsv) : [];

  const client = new Client({ connectionString: databaseUrl });
  await client.connect();

  try {
    if (!args.dryRun) {
      await client.query('BEGIN');
    }

    const adminId = await resolveAdminId(client, args.createdBy);

    let planIndexes;
    if (args.seedPlans) {
      const seedStats = await seedMissingPlans(client, planRows, adminId, args.dryRun);
      planIndexes = seedStats.indexes;
      // biome-ignore lint/suspicious/noConsole: CLI script
      console.log(
        `Catalog plans: ${seedStats.inserted} ${args.dryRun ? 'would insert' : 'inserted'}, ` +
          `${seedStats.skipped} already present`,
      );
    }

    // Always resolve catalog plan IDs from the database (never CSV).
    if (!args.dryRun || !args.seedPlans) {
      planIndexes = buildPlanIndexes(await loadPlans(client));
    }

    // biome-ignore lint/suspicious/noConsole: CLI script
    console.log(`Admin createdBy: ${adminId}`);
    // biome-ignore lint/suspicious/noConsole: CLI script
    console.log(`Plans in database: ${planIndexes.byId.size}`);

    if (args.usersCsv) {
      const userStats = await upsertUsers(client, userRows, adminId, args.dryRun);
      // biome-ignore lint/suspicious/noConsole: CLI script
      console.log(
        `Users: ${userStats.inserted} ${args.dryRun ? 'would insert' : 'inserted'}, ` +
          `${userStats.updated} ${args.dryRun ? 'would update' : 'updated'}, ` +
          `${userStats.skipped} skipped, ` +
          `${userStats.normalized} usernames normalized` +
          (userStats.duplicateUsernames > 0
            ? `, ${userStats.duplicateUsernames} duplicate usernames skipped`
            : ''),
      );
    }

    // Always resolve player IDs from the database (never CSV).
    const usernameIndex = await loadDbUsernameIndex(client);

    const stats = {
      inserted: 0,
      updated: 0,
      skipped: 0,
      errors: [],
    };

    for (const row of planRows) {
      const username = row.username?.trim();

      if (!username) {
        stats.skipped += 1;
        continue;
      }

      const playerId = usernameIndex.get(usernameLookupKey(username));
      if (!playerId) {
        stats.errors.push(`No user for username "${username}"`);
        stats.skipped += 1;
        continue;
      }

      const resolved = resolvePlanForCsvRow(row, planIndexes);
      if (!resolved.plan || !resolved.planId) {
        const scope = resolved.scope ?? csvPlanScope(row);
        stats.errors.push(
          `No catalog plan for "${username}" ` +
            `(name="${scope.name}", price=${scope.price}, ` +
            `deviceType=${scope.deviceType ?? 'null'}, deviceSubType=${scope.deviceSubType ?? 'null'})`,
        );
        stats.skipped += 1;
        continue;
      }

      try {
        const result = await upsertBalance(
          client,
          row,
          playerId,
          resolved.plan,
          adminId,
          args.dryRun,
          resolved.planId,
        );
        if (result.action === 'inserted') stats.inserted += 1;
        else if (result.action === 'updated') stats.updated += 1;
        else stats.inserted += 1;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        stats.errors.push(`${username}: ${message}`);
        stats.skipped += 1;
      }
    }

    if (!args.dryRun) {
      if (stats.errors.length > 0) {
        await client.query('ROLLBACK');
      } else {
        await client.query('COMMIT');
      }
    }

    // biome-ignore lint/suspicious/noConsole: CLI script
    console.log(
      `Plan balances: ${stats.inserted} ${args.dryRun ? 'would insert' : 'inserted'}, ` +
        `${stats.updated} ${args.dryRun ? 'would update' : 'updated'}, ` +
        `${stats.skipped} skipped` +
        (args.dryRun ? ' (dry-run)' : ''),
    );
    if (stats.errors.length > 0) {
      // biome-ignore lint/suspicious/noConsole: CLI script
      console.log('Issues:');
      for (const err of stats.errors) {
        // biome-ignore lint/suspicious/noConsole: CLI script
        console.log(`  - ${err}`);
      }
      process.exit(1);
    }
  } catch (error) {
    if (!args.dryRun) {
      await client.query('ROLLBACK');
    }
    throw error;
  } finally {
    await client.end();
  }
}

main().catch((error) => {
  // biome-ignore lint/suspicious/noConsole: CLI script
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
