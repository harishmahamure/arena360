# Tenant reporting with DuckDB

Operations commit to tenant SQLite. A canonical, secret-free snapshot is captured
with each outbox event in the same transaction. The owning cell publishes events
to NATS JetStream and consumes them into that tenant’s `analytics.duckdb` file.
PostgreSQL stores control metadata and global staff credentials.

## Setup

Follow [storage-cell development](storage-cell-development.md) to register the cell,
configure durable tenant storage, provision `ARENA_TENANT_EVENTS`, and select the
matching DuckDB SDK or bundled build. Production builds enable native analytics.
A default development build without `duckdb-analytics` serves operational APIs and
returns `503 ANALYTICS_UNAVAILABLE` for reports.

Run `pnpm demo:seed` after configuring the registered owning cell and JetStream.
The in-process consumer builds missing or incompatible projections from a consistent
SQLite snapshot, replays subsequent events, and switches the shadow file atomically.
No separate reporting service or legacy worker is required.

## Serving and correctness

Every `/stats/*` route, finance report, expense summary, credit portfolio summary,
inventory overview, receipt summary and waste summary reads tenant DuckDB.
Current local permission and venue grants are resolved before a reader or cache is
consulted. Fact venue snapshots keep historical usage at its original venue even
when a device moves; child records are constrained to their scoped parents.

Native reads are fenced and use parameterized SQL. Money stays DECIMAL until public
floating-point DTO presentation; finance exports return exact decimal strings.
Business calendar bounds use the tenant’s IANA timezone, including daylight-saving
changes. Finance exports retain their existing UTC date contract. Normal report
ranges must fit the hot window; retained monthly summaries survive fact retention.

Unready, rebuilding or restored-ahead projections return `503 ANALYTICS_UNAVAILABLE`
with `report temporarily rebuilding`. Cache identity includes tenant, ownership
generation, schema, timezone, hot boundary, ingestion checkpoint and selected venues.
Readiness and date bounds are checked before cached results. Reporting failures are
explicit; operational aggregate queries are never substituted.

Reports are eventually consistent. Each query uses a native read snapshot; multiple
queries composing a dashboard may observe intervening ingestion. A freshly read
SQLite watermark validates the sequence actually observed, so ordinary ingestion
advancing while a report waits does not resemble a restore-ahead condition.

## Recovery and verification

Monitor outbox age/count, publish failures, consumer lag, sequence gaps, rebuild
failures and background admission. NATS outages retain writes in SQLite. Duplicate
and older sequences cannot inflate aggregates; gaps stop ingestion and require a
consistent rebuild. The shadow rebuild preserves sealed monthly summaries for the
same calendar, catches up to a finite source watermark and switches only after all
checks pass. Operational writes remain available during rebuilding.

`pnpm backend:test:integration` runs native report parity when `DUCKDB_LIB_DIR` is
configured, plus disposable control and JetStream gates. See the development guide
for signed SQLite extensions and loader paths. `tests/report_parity.rs` verifies all
31 unchanged M0 fixtures using original input, fixed observation time, canonical UTC
normalization and a documented inventory tie boundary. Native tests also cover
scope, readiness, revocation, retention, exact money, refunds, delivery and recovery.

## Demo data

`pnpm demo:seed` provisions a tenant on a registered storage cell and writes operational
records through the same fenced SQLite commands used by the API. Configure
`CONTROL_DATABASE_URL`, `ARENA_CELL_ID`, and `TENANT_DATA_DIR` for that cell.
`DEMO_CONTROL_DATABASE_URL` can explicitly select a disposable control database.
`DATABASE_URL`, `DB_*`, and `DEMO_DATABASE_URL` are not seed targets.
The command loads `apps/backend/.env` without overriding process variables and requires
Node 20.12 or newer plus the backend Rust toolchain.

```bash
pnpm demo:seed --dry-run
pnpm demo:seed --date 2026-10-02 --tenant-slug arena360-demo
```

`--dry-run` prints the seed plan without connecting to a database or creating files.
The date selects the end of 60 UTC calendar dates and must not be in the future.
The operational dataset includes 32 fictional players, 12 PCs, three plans, five
products, 430 sales, 270 completed sessions, five active sessions, two pending kiosk
orders, partial credit settlements, kitchen tickets, expenses, stock receipts,
a partially received purchase order, reorder rules, and reconciled cash registers.
Every business command commits its own records, ledger effects, and canonical outbox
snapshots atomically. Owning cells publish the outbox through JetStream and build
the tenant DuckDB projection. Reports become available once its status is READY.

Global demo owner/counter accounts and local player accounts have unusable password
hashes by default. Set `DEMO_OWNER_USER_ID` (or `--owner-user-id`) to an existing active
control-plane operator to give that operator administrator membership in the demo tenant;
its credentials are preserved. Set `DEMO_PLAYER_PASSWORD` to an 8–72 byte password to
enable player login with usernames `demo.player.01` through `demo.player.32`.
Credentials are absent from command output and outbox events. No external payment or
notification delivery is performed.

The tenant file stores a completion marker. Repeating a completed seed returns the
original summary with `already-seeded`, without changing records or presentation dates.
An occupied target is rejected. Seeding spans multiple business transactions; an
interrupted run preserves its committed data and rejects another run under that slug.
Choose a fresh `arena360-demo...` slug for an interrupted run or a new date range.
Only ACTIVE tenants owned by the selected cell can be reopened.

`pnpm demo:test` checks the deterministic legacy report fixture generator and the Node
command adapter. The fixture generator remains available for M0 report baselines; the
new operational seed is version `arena360-demo-v2`. Native projection parity and
report serving are verified by the integration runner.
The production binary integration test checks real provisioning, service writes, wallet
and cash reconciliation, secret-free outbox payloads, credential preservation and repeat
behavior against temporary tenant files:

```bash
CONTROL_TEST_DATABASE_URL=postgres://.../isolated_control \
  cargo test --manifest-path apps/backend/Cargo.toml --test demo_seed -- --ignored
```

## Advanced analytics workspace

`/analytics` contains eleven report subpages. The sidebar and page search expose each
one; dates and comparison settings carry across subpages. Reports use
`GET /stats/business`, current `finance:read` permission and the selected venue scope.
Dates are inclusive tenant calendar days, bounded to 366 days, with an exclusive
observed end clipped to the current time. Reports cache for 30 seconds; the generated
timestamp is not an ingestion watermark. Current display contracts and metric
limitations are preserved by M0 parity tests.

### Metric definitions and limits

| Dashboard | Definition / supported action | Limits that remain visible in the UI |
| --- | --- | --- |
| Executive Overview | Completed + credit sales once, session starts, occupied hours, average ticket, repeat-visitor share | Credit collections are excluded. Capacity uses current inventory. Prepaid holders are a current snapshot. |
| Location Performance | Session hours, starts, and inventory grouped by device area label | These are not tenant/branch accounts. No branch revenue, ticket, or growth attribution without immutable venue IDs on facts. |
| Busy Hours & Capacity | Session intervals clipped to period and split at tenant-calendar hour boundaries; weekday/hour heatmap | Operating window is an assumption. Capacity = current inventory × elapsed selected operating hours. No waiting-demand events. |
| Revenue Opportunity | max(0, capacity − occupied hours) × assumed hourly rate × target fill share | Gross scenario, not observed lost revenue; excludes costs, downtime, and demand constraints. |
| Dynamic Pricing | Occupied-hour and price-change scenarios, capped by estimated capacity | No inferred price elasticity and no automatic live price changes. |
| Customer Retention | New = first recorded session in period; repeat = a prior session plus current visit; retention = prior-period visitors returning / prior-period visitors | Sessions are visits, not unique visit-days. Staff-allowance wallets excluded. At-risk rule: absent 30–89 days at period end. Detail is most recent 500 customers; summaries cover all. |
| Membership Analytics | Current active prepaid wallets with remaining minutes and future expiry; plan repeat purchases within selected period | Not subscriptions, MRR, or true renewals. Sold hours use current catalog credits; historical entitlement snapshots needed for exact utilization. |
| Station Performance | Recorded station hours/starts and current status; period plan sales allocated in proportion to hours | Revenue allocation is modeled; current status is not a downtime timeline. |
| F&B / POS | POS sales / session starts; distinct visitors who also bought POS in period / distinct session visitors; top 100 product line totals | Attach rate is customer-period, not session-level attribution. Legacy line totals can differ from sale adjustments. |
| Staff / Operations | Creator-attributed sales and session starts; sales / overlapping shift hours | No demand/role adjustment, and no inferred override/discount leakage. Missing shift hours produce unavailable ratios. |
| Forecast / Recommendations | Next 7 days after selected end, mean of complete matching weekdays including zero days; minimum 14 complete days | Historical low/high are not confidence intervals. No holiday/promotion/trend model or validated accuracy claim. Recommendations are explicit rules. |

Waiting demand needs arrival, wait, served/abandoned/rejected outcomes. Downtime
needs offline/online/maintenance intervals. Add immutable organization and venue attribution
to transactional facts before enabling multi-branch revenue comparisons; do not infer tenant
ownership from free-text station locations or inventory warehouse IDs.

Validation: `pnpm --filter @gaming-cafe/admin test -- src/pages/dashboard/analytics`
checks metric grains, scenario caps, forecasts, navigation and Apply semantics.
Native `business_analytics`, `location_reporting`, `report_cutover` and
`report_parity` tests cover report behavior without an external reporting database.
