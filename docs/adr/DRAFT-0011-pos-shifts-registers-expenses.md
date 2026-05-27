# ADR-0011: POS shifts, cash registers, expenses, and configuration

**Status**: Proposed
**Date**: 2026-05-27
**Deciders**: Platform team

## Context

The gaming cafe admin currently lacks foundational POS capabilities:

1. **Shifts** are tracked client-side only (localStorage timestamp set on
   staff login). There is no backend persistence, no clock-out, no handoff
   between staff, and shift data is lost on browser clear.
2. **Cash registers** do not exist. Cash vs online payments are tracked on
   individual transactions, but there is no opening float, denomination
   count, cash-in/out entries, or closing reconciliation.
3. **Expenses** are not tracked at all. The legacy NestJS expense entity
   was explicitly not ported (see `docs/PLANNER.md`). Net revenue cannot
   be calculated.
4. **Business configuration** (name, GST, receipt settings, pricing
   defaults) is hardcoded or missing entirely. App settings are env-only.
5. **Audit columns** (`created_by`, `updated_by`) do not exist on any
   table. There is no way to trace who created or last modified a record.
6. **Date handling** is inconsistent: frontend sends local calendar dates
   as `YYYY-MM-DD`, backend interprets them as UTC midnight, causing a
   5.5-hour offset for IST operations.

These gaps prevent the system from functioning as a production POS.

## Decision

We will add the following to the existing Rust Axum backend (ADR-0009):

### New database tables (8)

1. **`shifts`** — clock-in/clock-out records per staff user
2. **`cash_registers`** — one per shift, tracks opening/closing balances
   and denomination counts (JSONB)
3. **`cash_register_entries`** — individual cash-in/cash-out during a shift
4. **`cash_deposits`** — staff-initiated cash withdrawals pending admin
   approval (bank or home destination)
5. **`expense_categories`** — hierarchical expense categories with budgets
6. **`vendors`** — supplier/vendor directory
7. **`expenses`** — expense records with approval workflow
8. **`configurations`** — key-value business settings (JSONB values)

### Schema changes to existing tables (11)

Add `created_by UUID REFERENCES users(id)` and
`updated_by UUID REFERENCES users(id)` as nullable columns to all
existing tables: `users`, `devices`, `games`, `plans`, `player_plans`,
`units`, `device_games`, `usage_sessions`, `transactions`, `products`,
`files`. Backfill with the first admin user's ID.

### Schema changes to users (TOTP)

Add `totpSecret VARCHAR(64)` (nullable) and `totpEnabled BOOLEAN NOT NULL
DEFAULT false` to `users`. Admins configure RFC 6238 TOTP for staff accounts;
TOTP is required during shift handover validation.

### New API surface (~30 endpoints)

- `/shifts/*` — clock-in, clock-out, handover, list, get, active
- `/cash-registers/*` — open, close, entries, get, list
- `/cash-deposits/*` — initiate, list, get, approve, reject
- `/users/{id}/totp/*` — setup, verify, delete (admin-only)
- `/expense-categories/*` — CRUD
- `/vendors/*` — CRUD
- `/expenses/*` — CRUD + approve/reject + summary
- `/config/*` — list, get, upsert

### New permissions (10)

`shifts:read`, `shifts:write`, `cash-registers:read`,
`cash-registers:write`, `expenses:read`, `expenses:write`,
`expenses:approve`, `vendors:read`, `vendors:write`, `config:read`,
`config:write`

### Date handling fix

Backend stats queries will accept full RFC 3339 timestamps with timezone
offset. Frontend will send IST-aware timestamps instead of bare dates.

### Architecture

All new code follows the existing layered architecture:
`handlers/ → services/ → repositories/ → models/`

One new external crate is required: **`totp-rs`** for RFC 6238 TOTP
generation and verification (staff handover and admin-managed staff 2FA).
JSONB columns use `serde_json::Value` (already a dependency via `sqlx`).
Migrations use `sqlx migrate add`.

## Consequences

### Positive

- Complete shift lifecycle tracking with server-side persistence
- Cash reconciliation with denomination tracking and variance reports
- Full expense management with approval workflow and budget tracking
- Configurable business settings without redeployment
- Full audit trail via `created_by`/`updated_by` on every record
- Correct timezone handling for IST operations

### Negative

- 5 new database migrations to manage
- ~20 new API endpoints to maintain and test
- Increased schema complexity (7 new tables, 22 new columns on existing)
- `created_by`/`updated_by` adds a parameter to every service function

### Risks

- **Risk**: Migration on production database with existing data.
  **Mitigation**: All new columns are nullable; backfill runs in same
  transaction; new tables have no data dependencies on existing rows.
- **Risk**: Denomination JSONB schema may need to change (new currency
  notes introduced by RBI).
  **Mitigation**: JSONB is schemaless; frontend denomination grid is
  data-driven from a constant, not hardcoded fields.

## Alternatives considered

### Separate microservice for POS features

- Pros: Independent deployment, no schema coupling.
- Cons: Adds operational complexity for a single-location cafe;
  cross-service queries for stats; contradicts ADR-0009's monolith choice.
- **Why rejected**: Over-engineered for the current scale.

### Use environment variables for configuration

- Pros: No new table; familiar pattern.
- Cons: Requires redeployment to change settings; no audit trail;
  no per-user access control.
- **Why rejected**: Business settings change frequently (e.g., GST number,
  receipt text) and need UI-based management.

## References

- ADR-0009: Rust Axum backend (architecture this extends)
- ADR-0004: Shared API types via OpenAPI (new endpoints will be added)
- ADR-0008: Runtime contracts (new permissions and error codes)
- DRAFT-0010: Transaction product line items (related POS work)
