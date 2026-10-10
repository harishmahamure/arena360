# Tenant reporting with SQLite

Phase 1 serves operations and every tenant report from the same SQLite file over
private gRPC. No DuckDB library, analytics rebuild, NATS server or ingest lag is
required. See the [architecture](network-database-service.md) and
[deployment guide](../operations/two-rust-services.md).

## Serving and correctness

The operational schema is STRICT. UUIDs and canonical UTC timestamps are TEXT,
money is scale-4 INTEGER, booleans are checked INTEGERs, and structured settings
are validated JSON TEXT. Migration `0018_sqlite_reporting.sql` adds ordinary,
secret-free views over those tables; there are no duplicated reporting tables or
asynchronously maintained summaries in Phase 1.

All `/stats/*` routes, finance exports, expense summaries, credit portfolio,
inventory overview, receipt summaries and waste summaries use a report reader.
The adapter checks current ownership before and after queries and resolves venue
grants before reading or consulting the response cache. Historical facts retain
their venue and player snapshots. Empty grants return empty aggregates; they do
not grant tenant-wide access.

Queries within a dashboard share one read-only transaction. Money stays INTEGER
through sums and exact finance formatting; public numeric dashboard fields retain
their existing display contract. Business dates use the tenant's IANA calendar;
hourly occupancy clips session intervals and handles repeated/skipped DST hours.
Finance exports keep their UTC date contract. Unused stations have zero hours.

Time-range indexes cover transaction creation/occurrence, settlements, deposits,
reconciliation, expenses and receipts. Existing parent and venue indexes are
reused. The query-plan regression tests check indexed searches through the views.
Calendar functions appear in grouping/output, not in time-range predicates.
Full-history customer and current-wallet aggregates still read their relevant
history; measure those separately when deciding on future central analytics.

## Resource limits and events

There are two report snapshots per tenant and eight per worker. A snapshot expires
after 25 seconds; the SQLite progress hook interrupts long SQL and expiry releases
the connection even if a caller retains the reader. Results are bounded to 10,000
rows and 4 MiB per query. Busy/expired reports return `503 ANALYTICS_UNAVAILABLE`;
oversized results require a narrower request. Long read transactions cannot pin
WAL indefinitely. Reports use the existing short response-cache TTLs.

Versioned outbox events remain transactional. With optional NATS, durable publisher
acknowledgments and the realtime cursor govern deletion. Without NATS, age-based
local retention deletes only realtime-projected events, never operational rows.
Future centralized analytics must bootstrap a consistent SQLite snapshot before
consuming subsequent events. The optional legacy `duckdb-analytics` feature is
reserved for historical compatibility; default production builds use SQLite.

## Validation

`cargo test -p arena360-core --test report_parity --test report_cutover --test sqlite_reporting --test storage_rpc`
checks all 31 captured report contracts, authorized HTTP routes, revocation,
venue/tenant isolation, exact ledger totals, concurrent-write snapshots, migration
views/index plans and network reporting. `cargo test -p sqlite-reporting` checks
calendar/money functions, numeric named binding order, admission, read-only
statements, timeouts and result size. Session-hour unit tests cover IST and DST.

## Demo data

`pnpm demo:seed` provisions a tenant on a registered storage cell and writes operational
records through the same fenced SQLite commands used by the API. Configure
`CONTROL_DATABASE_URL`, `ARENA_CELL_ID`, and `TENANT_DATA_DIR` for that cell.
`DEMO_CONTROL_DATABASE_URL` can explicitly select a disposable control database.
`DATABASE_URL`, `DB_*`, and `DEMO_DATABASE_URL` are not seed targets.
The command loads the repository root `.env` without overriding process variables and requires
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
snapshots atomically. Reports read the committed tenant SQLite data immediately; JetStream is optional.

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
new operational seed is version `arena360-demo-v2`. SQLite report parity and report serving are verified by the integration runner.
The production binary integration test checks real provisioning, service writes, wallet
and cash reconciliation, secret-free outbox payloads, credential preservation and repeat
behavior against temporary tenant files:

```bash
CONTROL_TEST_DATABASE_URL=postgres://.../isolated_control \
  cargo test -p arena360-tools --test demo_seed -- --ignored
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
Default `sqlite_reporting`, `report_cutover`, `storage_rpc` and `report_parity` tests
cover report behavior without an external reporting database. The golden fixture
comparison corrects the captured M0 unused-station occupancy bug from the original
session inputs and retains the documented inventory tie-break rule.
