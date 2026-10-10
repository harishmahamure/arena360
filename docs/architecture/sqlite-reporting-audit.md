# Phase 1 SQLite reporting audit

The existing API provides executive, staff, finance, credit, expense and inventory
reports, plus eleven business analytics screens. All 31 captured report outputs
are exercised against migrated operational SQLite, with current venue permissions
and unchanged response contracts. This validates behavior on the fixture workload,
not a production tenant capacity estimate.

## Schema fit

| Data | SQLite representation | Decision |
| --- | --- | --- |
| Operational entities and ledgers | Existing STRICT tables | Retain as the authoritative data; reports reuse them. |
| Money | Checked scale-4 INTEGER | Sum in integer units; finance uses exact decimal strings. Dashboard floats are presentation values. |
| Instants / UUIDs | Checked canonical UTC TEXT / UUID TEXT | Raw timestamp range comparisons use indexes; labels use the tenant calendar. |
| Boolean / structured fields | Checked INTEGER / validated JSON TEXT | Preserve existing constraints and typed repository conversions. |
| Report projections | Ordinary `report_base_*` views | Explicit secret-free columns and stable reporting names; no extra data copies or refresh jobs. |
| Report authorization | Request CTEs | Venue filters, child-parent scope and historical attribution stay inside each pinned snapshot. |
| Daily/hourly summaries | Not introduced | Direct queries fit the current contract; adding summary tables would add transactional maintenance and repair requirements. |
| Future centralized analytics | Snapshot plus versioned events | Retained outbox is bounded, so bootstrap from a consistent source snapshot. |

The operational schema already had transaction occurrence, player, shift, venue,
stock parent and session-start indexes. Migration 0018 fills actual report gaps:

| Added index | Query served |
| --- | --- |
| `transactions_created_live` | Dashboard revenues, transaction averages, trend and top-plan creation-date windows |
| `credit_settlements_time_live` | Collections and collection trends across players |
| `cash_deposits_created` | Deposit totals across statuses |
| `cash_registers_created` | Reconciliation windows across statuses |
| `cash_registers_variance_time` | Closed/reconciled variance by update window |
| `expenses_date_live` | Ledger date windows independent of category |
| `usage_sessions_end_live` | Closed/open interval overlap and current read scope |
| `shifts_clock_out` | Overlapping shifts, including open shifts |
| `stock_receipts_created` | Receipt reporting's creation timestamp contract |

Range predicates use original columns rather than date/timezone functions.
`NOT MATERIALIZED` report CTEs allow predicate pushdown through the views. Core
query-plan tests check indexed searches through five principal report views.
Catalog/current-wallet counts and lifetime customer history intentionally retain
broader scans; they must be measured on representative large tenants before
claiming a scale ceiling or adding maintained rollups.

## Correctness changes

The captured station query credited a full selected interval to unused devices:
DuckDB's NULL handling let its left join produce a synthetic occupied interval.
Both reporting implementations now give an unused station zero occupancy.
Golden comparisons derive corrected station hours from the immutable session
inputs; the historical fixture files remain unchanged.

SQLite money aggregates retain precision beyond IEEE-754's exact integer range.
Split credit collections use decimal arithmetic before public floating-point
presentation. Finance rounding preserves the two/four decimal public contract.
All queries in a dashboard share one source snapshot; fresh readers see subsequent
commits and a new cache sequence. Ownership loss rejects reads, and empty venue
grants do not turn into tenant-wide access.

Resource admission, SQL deadlines, snapshot expiry and result limits live in the
independent `sqlite-reporting` crate. WAL snapshots are released after expiry even
if a caller retains a reader. There is no unbounded asynchronous analytics ingest
queue in default reporting. Optional integration NATS retains its acknowledged
publication contract, while no-broker outbox expiry waits for realtime projection.
