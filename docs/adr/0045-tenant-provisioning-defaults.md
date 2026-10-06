# ADR-0045: Tenant provisioning bootstrap schema and defaults

**Status**: Accepted (2026-10-06, owner)
**Date**: 2026-10-06
**Deciders**: Founder / backend owner
**Extends**: ADR-0043 and ADR-0044

## Context

M3 `API-0022` provisions a tenant by registering it in the control plane, creating its SQLite
file, applying all tenant migrations, seeding units, settings, and roles, and acquiring the
owning cell's lease.

ADR-0044 deliberately limits `0001_foundation.sql` to the transactional outbox and states that
business-domain tables arrive later. The provisioning task nevertheless requires durable homes
for its default rows before M4 repository ports exist. Adding those tables is a tenant database
migration and therefore needs an explicit decision.

Provisioning can fail between PostgreSQL, the local filesystem, SQLite migration/seed work, and
lease acquisition. These systems cannot share one transaction, so retries must converge without
duplicating control-plane rows, files, or defaults.

## Decision

1. Add `apps/backend/migrations/tenant/0002_provisioning_defaults.sql` containing only the
   tenant-local bootstrap tables needed before M4:
   - `units`, with canonical unit type, name, abbreviation, active state, and audit timestamps;
   - `setting_overrides`, for explicit tenant or location overrides only;
   - `access_roles`, for system and template role definitions.
2. The new tables follow ADR-0044 storage conventions and contain no `tenant_id` or
   `organizationId`. IDs are canonical UUID text, timestamps are fixed-width UTC text, booleans
   are constrained integers, and JSON values are validated text.
3. Platform setting defaults remain code-owned in the settings catalog. Provisioning seeds only
   explicit initial overrides supplied by the provisioning request; it does not copy every
   platform default into SQLite. Tenant time zone remains authoritative in PostgreSQL under
   ADR-0043 decision 28 and is not duplicated as a tenant setting.
4. Rust owns the versioned seed catalog. The initial catalog contains:
   - the eleven canonical units already used by the product (`piece`, `box`, `carton`, `pack`,
     `bottle`, `can`, `kilogram`, `gram`, `liter`, `milliliter`, `other`);
   - the current system `admin` and `staff` roles and the existing manager, counter, kitchen,
     finance, and auditor templates;
   - caller-supplied setting overrides validated by the settings catalog.
5. Seed rows use stable UUIDs and natural unique keys. Inserts are idempotent and never overwrite
   a tenant customization during a provisioning retry.
6. Provisioning is a resumable state machine:
   1. validate input and create or recover the PostgreSQL tenant in `PROVISIONING`;
   2. create the tenant directory and SQLite file without replacing an existing file;
   3. apply all embedded tenant migrations;
   4. seed defaults in one SQLite transaction;
   5. acquire the lease for the requested active cell;
   6. record the migrated schema version and mark the tenant `ACTIVE`.
7. A failure leaves the tenant non-active and returns the failed stage. Retrying the same
   provisioning key resumes the existing tenant and file. Cleanup never deletes a pre-existing
   tenant file automatically.
8. Only an `ACTIVE` cell may provision. The lease generation acquired during provisioning is the
   generation subsequently used by `TenantDbManager`.

## Consequences

### Positive

- A newly provisioned tenant is immediately usable by the upcoming M4/M5 repository ports.
- Retries converge across PostgreSQL, SQLite, the filesystem, and lease acquisition.
- Stable seeds avoid duplicate units and roles without overwriting customer changes.
- Settings retain one platform-default source of truth.

### Negative

- M3 introduces three small business-facing tables earlier than ADR-0044 originally anticipated.
- Provisioning is a multi-system workflow rather than one atomic transaction.
- The role permission catalog is duplicated from the transitional PostgreSQL implementation until
  operational cutover removes the old schema.

## Alternatives

### Defer all defaults to M4 and M5

Provision only the file, outbox, and lease in M3. This avoids a new tenant migration now, but does
not satisfy `API-0022` and leaves new tenants unusable until later schema tasks.

### Put bootstrap defaults in PostgreSQL

This avoids early SQLite tables, but makes business operations depend on the control plane and
conflicts with ADR-0043's tenant-local operational data boundary.

### Seed defaults in SQL migrations

This is compact, but mixes schema evolution with tenant initialization, makes caller-supplied
settings awkward, and makes preserving tenant customizations across retries less explicit.

## Risks

- A crash after lease acquisition but before activation can leave a leased `PROVISIONING` tenant.
  - Mitigation: retries inspect both control state and current lease; the normal lease expiry and
    self-fencing rules still apply.
- Seed constants drift from permission checks or the settings catalog.
  - Mitigation: contract tests compare seeded system keys and permissions to the shared Rust
    catalog.
- A retry targets a path containing an unrelated file.
  - Mitigation: require the expected tenant path, validate its SQLx migration metadata, and never
    truncate or replace it.

## References

- `docs/adr/0043-storage-cells-sqlite-duckdb.md`
- `docs/adr/0044-tenant-sqlite-baseline.md`
- `docs/plans/data-platform-build-plan.md` (`API-0022`)
- `docs/architecture/data-platform.md` (§7, §13, §51–55)
