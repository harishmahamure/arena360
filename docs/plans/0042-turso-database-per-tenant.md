# Plan: Turso database per tenant

Implements `docs/adr/DRAFT-0042-turso-database-per-tenant.md`. Do not start Phase 1 until the ADR is accepted.

## Goal

Every organization's operational data lives in its own Turso database, global identity lives in a control-plane database, and the analytics pipeline, realtime delivery, and reports keep working unchanged from the user's point of view.

## Out of scope

- Moving ClickHouse, NATS JetStream, or Redis off the VM.
- Cross-tenant platform reporting beyond what ClickHouse already provides.
- Offline writes from venue devices (embedded replicas are a later decision).

## Phase 0: Spike (1–2 weeks)

Validate effort, latency, and cost before committing.

- [ ] SPIKE-01: Create a Turso Free account with one control-plane database and two tenant databases. — XS
- [ ] SPIKE-02: Port the sessions and transactions tables to a SQLite schema using the ADR type mapping (UUID v7 text, RFC 3339 text timestamps, scale-4 integer money, JSON text). — S
- [ ] SPIKE-03: Port `session_repo.rs` and `transaction_repo.rs` to the `libsql` crate behind a tenant connection. — M
- [ ] SPIKE-04: Prototype tenant-routing middleware: JWT organization → registry lookup → tenant handle. — S
- [ ] SPIKE-05: Measure p50/p95 latency from the VM region for session start, session end, and checkout. — XS
- [ ] SPIKE-06: Measure rows read and written per session and per checkout from Turso usage, and recompute the cost estimate. — XS

**Exit criteria:** p95 latency for session start and checkout under 150 ms; measured cost per venue under $1.50 per month at 20 PCs; porting rate extrapolates to under 10 developer-weeks. If any fail, revisit the ADR before Phase 1.

## Phase 1: Foundations (about 2 weeks)

- [ ] DB-01: Define the control-plane schema: users, organizations, memberships, auth challenges, device routing, tenant registry (`organization_id`, `database_name`, `database_url`, `schema_version`, `status`, timestamps). — S
- [ ] DB-02: Write the tenant SQLite baseline covering all 76 current tables, dropping `organizationId` columns (the database is the tenant) while keeping location scoping. — L
  - Split into one task per domain: catalog and pricing; devices and sessions; transactions and ledger; inventory; shifts, cash, and expenses; settings, notifications, and outboxes.
- [ ] API-01: Add the database abstraction in `apps/backend`: control connection, tenant connection cache (bounded, keyed by organization), and the `libsql` dependency. — M
- [ ] API-02: Migration runner with two embedded sets (`migrations/control/`, `migrations/tenant/`) and a per-database migrations ledger. — M
- [ ] API-03: Migration orchestrator: apply pending tenant migrations to every registered tenant, record versions, retry failures, report drift. Runs as a CLI command and on deploy. — M
- [ ] API-04: Tenant provisioning: create the Turso database through the Platform API, apply migrations, seed defaults (units catalog, settings, roles), register in the control plane. — M
- [ ] API-05: Tenant-routing middleware for user and device JWTs, with registry caching in memory and Redis; reject requests for unknown, suspended, or out-of-date tenants. — M
- [ ] TEST-01: Money conversion helpers with property tests (Decimal ↔ scale-4 integer, rounding, negative values, max range). — S

**Exit criteria:** a new organization can be provisioned end to end; the orchestrator migrates N tenants and recovers from an injected failure; routing rejects cross-tenant tokens.

## Phase 2: Port repositories and services (about 4–5 weeks)

Port one domain at a time, keeping tests green after each.

- [ ] API-10: Identity and access (`user_repo`, auth service, memberships, access routes) against the control plane. — M
- [ ] API-11: Catalog and pricing (`product_repo`, `plan_repo`, `pricing_policy_repo`, `unit_repo`, `product_recipe_repo`, `game_repo`). — M
- [ ] API-12: Devices and sessions (`device_repo`, `session_repo`, `player_plan_repo`, `balance_repo`), including dynamic plan deduction. — L
- [ ] API-13: Transactions and ledger (`transaction_repo`, `transaction_product_repo`, `ledger_repo`, `credit_repo`, `kiosk_order_repo`). Replace `FOR UPDATE` with `BEGIN IMMEDIATE` transactions. — L
- [ ] API-14: Inventory (`inventory_repo`, `vendor_repo`). — M
- [ ] API-15: Shifts, cash, and expenses (`shift_repo`, `cash_register_repo`, `cash_deposit_repo`, `expense_repo`, `expense_category_repo`). — M
- [ ] API-16: Settings, configuration, and notifications (`settings_repo`, `config_repo`, `notification_repo`), replacing the advisory lock usage in `settings_repo`. — S
- [ ] API-17: Move logic from the 13 `plpgsql` functions and 78 triggers into repository code or single-statement SQLite triggers; keep audit columns and `updatedAt` behaviour identical. — M
- [ ] API-18: Remove the shared-table compatibility defaults and the non-default-organization block in `access/routes.rs`. — S

**Exit criteria:** all backend integration tests pass against Turso (or local libSQL files in CI); no remaining `PgPool` references outside the analytics migration tooling.

## Phase 3: Realtime and analytics (about 1–2 weeks)

- [ ] API-20: Replace `PgListener` in `realtime/dispatcher.rs` with Redis pub/sub published after commit; keep the per-tenant `realtime_outbox` for replay. — M
- [ ] API-21: Analytics outbox writes in each tenant transaction, carrying the same allowlisted fields as today. — M
- [ ] API-22: Analytics worker iterates active tenants, polls each outbox every 5–10 seconds, adds the organization ID, publishes to JetStream, and deletes rows after acknowledgement. — M
- [ ] API-23: Replace the PostgreSQL advisory lock that keeps one worker active with a Redis lease. — S
- [ ] API-24: Backfill command that snapshots every tenant into ClickHouse at version zero, reusing the existing readiness markers. — S
- [ ] TEST-20: Port the opt-in pipeline tests (duplicates, out-of-order delivery, tombstones, restart recovery) to the multi-tenant worker. — M

**Exit criteria:** reports for two tenants match their source data; killing the worker mid-batch loses and duplicates nothing.

## Phase 4: Cutover (about 1 week)

- [ ] OPS-01: One-time export tool from the current PostgreSQL database into the bootstrap organization's Turso database, converting types per the ADR. — M
- [ ] OPS-02: Reconcile exported data: row counts per table, money totals per ledger, wallet and stock balances. — S
- [ ] OPS-03: Run the ClickHouse backfill into a fresh database and switch the API to it after readiness. — XS
- [ ] OPS-04: Upgrade Turso to the Developer plan before the first paying venue. — XS
- [ ] OPS-05: Alerts for Turso rows read and written, outbox age per tenant, migration drift, and provisioning failures. — S
- [ ] DOC-01: Replace `docs/architecture/tenancy.md` with the database-per-tenant model and update `docs/architecture/analytics.md` for the per-tenant outbox. — S
- [ ] DOC-02: Mark the ADR accepted and record the outcome in `docs/adr/.events.jsonl`. — XS

**Exit criteria:** demo and bootstrap data reconcile exactly; PostgreSQL is no longer required by the API.

## Estimate

| Phase | Duration |
|---|---|
| 0. Spike | 1–2 weeks |
| 1. Foundations | ~2 weeks |
| 2. Port repositories | ~4–5 weeks |
| 3. Realtime and analytics | ~1–2 weeks |
| 4. Cutover | ~1 week |
| **Total** | **~9–12 weeks for one developer** |

Assumptions: one developer full time; Turso Free is enough until cutover; no new features ship during Phase 2.

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Porting takes longer than estimated | Medium | High | Phase 0 measures the real rate; stop and revisit if it exceeds 10 weeks |
| Money calculations differ after the type change | Medium | High | TEST-01 property tests and OPS-02 reconciliation before cutover |
| Turso latency from the VM region is too high | Low | High | SPIKE-05; choose the closest Turso region to the VM |
| Migration fans out with partial failures | Medium | Medium | API-03 per-tenant versioning and retries |
| Outbox polling becomes expensive at many tenants | Low | Medium | Indexed outbox, 5–10 second interval, poll only active tenants |
