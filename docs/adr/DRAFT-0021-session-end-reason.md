# ADR-0021: Session end reason persistence

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team (gaming-cafe kiosk working group)

**Relates to**: [ADR-0009](0009-rust-axum-backend.md) (backend layering, SQLx migrations), [ADR-0018](0018-kiosk-ws-device-acl.md) (`session.ended` event payload)

## Context

[PLANNER-KIOSK.md](../PLANNER-KIOSK.md) task `be-session-end-reason-enum` and
[REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) §7.3 require the backend to
record and broadcast *why* a session ended so operators and the kiosk can react
differently:

| Reason | Trigger |
|--------|---------|
| `voluntary` | Player pressed "End session" |
| `auto` | Timer hit zero (`ENDED_AUTO`) |
| `force` | Staff/admin force-ended from the admin SPA |
| `offline_reconcile` | Kiosk replayed a queued end after reconnect (US-KOFFLINE-003) |

The `usage_sessions` table (see `apps/backend/src/models/session.rs`) has no
column for this today. Adding one is a **new migration** in
`apps/backend/migrations/`, which per `.cursor/rules/20-adr-discipline.mdc`
requires an ADR before the schema change lands.

## Decision

Add a nullable text column `endReason` to `usage_sessions`:

```sql
-- 20260530000005_session_end_reason.sql
ALTER TABLE usage_sessions
  ADD COLUMN IF NOT EXISTS "endReason" VARCHAR(32);
```

- Nullable, no default — historical rows and in-progress sessions stay `NULL`
  (KISS; no backfill, no enum type churn).
- Application-level validation restricts values to the four reasons above; the
  column is plain `VARCHAR` rather than a PG enum so we can add reasons later
  without a second migration (consistent with how other status strings are
  validated in the service layer).
- `EndSessionDto` gains an optional `reason` field; `SessionService::end`
  persists it and includes it in the `session.ended` WebSocket payload
  (`{ sessionId, deviceId, reason }`, camelCase per ADR-0018 / D15).
- `UsageSession` / `UsageSessionResponse` expose `endReason`.

A matching `.down.sql` drops the column.

## Consequences

### Positive

- Operators see end reason in session history; kiosk can distinguish auto-end
  from force-end (5-min grace UI) from a single event field.
- No enum type migration; future reasons are a code change only.

### Negative

- One more nullable column on a hot table (negligible storage).
- Service layer, not the DB, enforces the allowed value set.

### Risks

| Risk | Mitigation |
|------|------------|
| Inconsistent reason strings from clients | Validate against a fixed set in `SessionService::end`; reject unknown with 400 |
| Old clients omit reason | Column nullable; event omits field when absent |

## Alternatives considered

### A. Native PostgreSQL enum type

- Pros: DB-enforced domain.
- Cons: Adding a value requires `ALTER TYPE` migration; heavier. **Rejected** for v1.

### B. Derive reason from audit log / no persistence

- Pros: No schema change.
- Cons: Can't query history efficiently; event payload still needs the value at
  end time. **Rejected** — persistence is the requirement.

## Implementation notes

After **Acceptance**:

1. Add migration `20260530000005_session_end_reason.sql` (+ `.down.sql`).
2. Extend `EndSessionDto`, `UsageSession`, `UsageSessionRow`, `UsageSessionResponse`.
3. Persist in `SessionRepository::end`; validate + emit in `SessionService::end`.
4. Regenerate `@gaming-cafe/api-types`.

Until accepted, `SessionService::end` accepts `reason` and echoes it into the
`session.ended` event **without** persisting it (no schema dependency).

## References

- [ADR-0009](0009-rust-axum-backend.md)
- [ADR-0018](0018-kiosk-ws-device-acl.md)
- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) §7.3
- [PLANNER-KIOSK.md](../PLANNER-KIOSK.md) — `be-session-end-reason-enum`
