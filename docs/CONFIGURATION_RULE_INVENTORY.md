# Arena360 configuration ownership

This inventory defines where a value belongs. It prevents the scoped venue
settings API from becoming a second environment-variable store or an
unvalidated rule engine.

| Ownership class | Examples | Storage and change path |
| --- | --- | --- |
| Code invariant | positive quantities, payment arithmetic, enum validity, referential integrity, four-digit payment references | Rust/domain types and database constraints; changed through a reviewed release |
| Platform/runtime | request deadlines, connection pools, queue sizes, cache TTLs, object-storage endpoints, public routes | environment or deployment configuration; never returned by venue settings APIs |
| Venue setting | identity and receipt text, timezone, currency, tax, plan defaults, staff allowance period, session warnings, notification retention, default inventory locations | typed catalog plus organization/location overrides and immutable revisions |
| Domain rule | pricing schedules, inventory reorder rules, plan eligibility/deduction, future membership/reservation/loyalty/approval rules | dedicated typed domain records and evaluators with lifecycle controls |
| Secret | JWT signing keys, database/Redis credentials, payment and email provider credentials | secret manager/environment only; never stored in setting values or audit payloads |

## Initial migrations

- `CAFE_TZ` remains the platform bootstrap default, while effective venue
  behavior resolves `venue.timezone`.
- The legacy `23:00–08:00` product price window resolves
  `pricing.night_window_start` and `pricing.night_window_end`, then runs through
  the typed decimal pricing evaluator while retaining legacy numeric API fields.
- Plan creation resolves `plans.default_validity_days` and
  `plans.default_time_credits`.
- Staff allowance grants resolve `staff.allowance_period_days`.
- Existing `business.*`, `receipt.*`, `pricing.*`, inventory default-location,
  and POS default-location keys are backfilled as organization overrides.

## Rules for new code

1. Business services consume typed resolver methods, not raw JSON or direct
   `setting_overrides` queries.
2. Every new catalog key must declare a type, default, owner, allowed scope,
   validation, and tests.
3. Values involving conditions, priorities, time ranges, or state transitions
   are domain rules rather than scalar settings.
4. Published rules are immutable. Rollback creates and publishes a new version.
5. Security and infrastructure values must not be added to the venue catalog.
