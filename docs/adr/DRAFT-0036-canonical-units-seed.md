# DRAFT-0036: Seed canonical product units

**Status**: Proposed
**Date**: 2026-06-15
**Deciders**: Platform team

**Relates to**: [DRAFT-0032](DRAFT-0032-inventory-locations-and-pos-pricing.md)

## Context

Product sale and purchase units reference `units(id)` UUIDs. The admin product form
loads options from `GET /units`, but fresh databases have no unit rows — dropdowns
appear empty. Operators were expected to create units manually; we want a fixed
catalog of 11 standard types (piece, box, bottle, etc.) with no custom unit CRUD in admin.

## Decision

1. Seed one active row per `UNIT_TYPES` value via idempotent SQL migration.
2. Add partial unique index on `units(type)` where `"deletedAt" IS NULL`.
3. Expose `CANONICAL_UNITS` in `@gaming-cafe/contracts` for label ordering.
4. Product form maps canonical types to unit UUIDs; defaults: sale = piece, purchase = box.
5. Remove Units management from admin nav and routes; keep read-only `GET /units` for forms.

## Consequences

### Positive

- Product create/edit always shows full unit list without manual setup.
- Single canonical catalog aligned with `UnitType` enum.

### Negative

- Custom units created before this change remain in DB but are hidden from product form.
- New DB migration required on deploy.

## Alternatives Considered

### Accept unit type string on products (no FK)

Rejected: requires API and schema change beyond seed data.

### Runtime upsert in `GET /units`

Rejected: migration is clearer for production and CI.

## References

- `packages/contracts/src/enums.ts` — `UnitType`, `CANONICAL_UNITS`
- `apps/backend/migrations/20260615000001_seed_canonical_units.sql`
