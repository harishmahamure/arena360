# ADR-0044: Tenant SQLite baseline and outbox schema

**Status**: Accepted (2026-10-06, owner)
**Date**: 2026-10-06
**Deciders**: Founder / backend owner
**Implements**: ADR-0043 decisions 7 and 12–20

## Context

ADR-0043 selects one SQLite database per tenant and defines the storage conventions, but the
repository does not yet have a tenant migration. M3 `DB-0010a` needs a minimal schema that fixes
the on-disk representation before business tables are added in M4 and M5.

The baseline must preserve physical tenant isolation, deterministic ordering, exact numeric
storage, UTC-only timestamps, valid JSON, and a transactional outbox that can feed JetStream and
DuckDB. It must not copy the legacy shared-table `organizationId` tenancy model into each file.

## Decision

1. Tenant migrations live in `apps/backend/migrations/tenant/`, beginning with
   `0001_foundation.sql`. The migration is transactional and is applied independently to every
   tenant file.
2. All tenant schemas use these representations:
   - IDs: UUID v7 generated in Rust and stored as canonical 36-character `TEXT`.
   - Instants: fixed-width UTC `TEXT` in `YYYY-MM-DDTHH:MM:SS.ffffffZ` form.
   - Money and decimal quantities: scale-4 `INTEGER`.
   - Boolean values: `INTEGER` constrained to `0` or `1`.
   - JSON: `TEXT` constrained by `json_valid`; object payloads additionally require
     `json_type(value) = 'object'`.
3. Tenant business tables never contain `organizationId` or `tenant_id`. The SQLite file is the
   tenant boundary. Location-scoped tables may contain `location_id`.
4. The baseline creates `outbox_events`:
   - `sequence INTEGER PRIMARY KEY AUTOINCREMENT`;
   - `event_id`, `location_id`, `aggregate_type`, `aggregate_id`, `event_type`;
   - `occurred_at`, `schema_version`, `deleted`, and `payload`.
5. `tenant_id` is added to the JetStream event envelope by the publisher from its trusted
   `TenantDb` context. It is not duplicated in every SQLite outbox row.
6. Outbox rows are inserted in the same SQLite transaction as the business change. They are
   deleted only after JetStream acknowledges publication.
7. The baseline contains no business-domain tables. Those arrive in the bounded M4/M5 schema
   tasks, following these conventions.

## Consequences

### Positive

- The file itself remains the tenant isolation boundary.
- Lexical timestamp order equals chronological order.
- Money remains exact and JSON corruption is rejected at write time.
- SQLite's single writer and `AUTOINCREMENT` make outbox sequence equal commit order.
- The baseline is small enough to validate independently before porting business repositories.

### Negative

- UUID and timestamp checks validate canonical shape, while semantic generation/parsing remains a
  Rust responsibility.
- Fixed-width microsecond timestamps require one canonical formatter at every write boundary.
- The publisher must enrich persisted outbox rows with the tenant ID.

### Risks

- A repository bypasses shared conversion helpers and writes a malformed value.
  - Mitigation: schema checks plus property and migration tests in `API-0023`.
- `AUTOINCREMENT` adds a small SQLite bookkeeping cost.
  - Mitigation: accept it because never reusing an outbox sequence is more important.
- Future migrations accidentally reintroduce shared-table tenancy.
  - Mitigation: a schema contract test rejects `organizationId` and tenant discriminator columns.

## Alternatives Considered

### Store UUIDs as 16-byte BLOBs

- More compact, but harder to inspect and inconsistent with the explicit ADR-0043 text convention.

### Store timestamps as Unix integers

- Compact and fast, but not self-describing and conflicts with the accepted fixed-width UTC text
  decision.

### Persist the entire event envelope as one JSON document

- Flexible, but weakens constraints and makes publisher scans and debugging less reliable.

### Include `tenant_id` in every outbox row

- Makes rows self-contained, but duplicates the physical tenant boundary and risks disagreement
  between file identity and row data.

## Implementation Notes

- The canonical timestamp shape is 27 bytes, for example `2026-10-06T05:01:00.000000Z`.
- `event_id` and aggregate IDs are generated before entering the transaction.
- No SQLite trigger writes outbox events; Rust repositories explicitly write them in their
  transaction so event payload construction remains testable.
- Migration tests open an empty temporary SQLite file, apply the migration, inspect the schema,
  and verify every constraint with accepted and rejected inserts.

## References

- `docs/adr/0043-storage-cells-sqlite-duckdb.md`
- `docs/architecture/data-platform.md`
- `docs/architecture/duckdb-analytics-schema.md`
- `docs/plans/data-platform-build-plan.md` (`DB-0010a`)
