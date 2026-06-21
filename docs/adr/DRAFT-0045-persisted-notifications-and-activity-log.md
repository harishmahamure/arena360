# ADR-0045: Persisted notifications and activity log

**Status**: Proposed
**Date**: 2026-06-21
**Deciders**: Platform team

**Extends**: [ADR-0013](0013-realtime-websocket-channel.md) (adds user-facing persistence on top of existing WS transport)

## Context

ADR-0013 delivers real-time events via WebSocket and a Postgres outbox (`realtime_outbox`). The admin panel consumes these as ephemeral toasts and React Query invalidations. There is no:

- Per-user notification inbox with read/unread state
- Queryable activity history for important operational actions
- Persisted delivery for staff (approvals, settlements, shift events) beyond transient WS frames

The outbox retains rows 7–30 days for transport replay only; it is not designed as a user-facing audit feed.

## Decision

1. **Dual-write pattern** — Domain services call `NotificationService::record`, which:
   - Inserts an `activity_log` row (org-wide history)
   - Inserts `user_notifications` rows for resolved recipients
   - Publishes `notification.created` via existing `OutboxService` to `user:{id}` channels

2. **Schema** — New tables `activity_log` and `user_notifications` with `activity_kind` enum cataloguing important actions (sales, approvals, sessions, credit settlements, shifts, inventory, etc.).

3. **REST API** — Admin/staff JWT endpoints:
   - `GET /notifications`, `GET /notifications/unread-count`
   - `PATCH /notifications/{id}/read`, `POST /notifications/read-all`
   - `GET /activity-log` (admin: all; staff: scoped to their feed)

4. **Recipient rules** — Admins receive operational alerts (staff sales, approval requests). Staff receive decisions on their requests, their own actions (inventory submit, settlements), and shared floor events (sessions, devices). Role resolution queries active admin users at record time.

5. **Backward compatibility** — Existing outbox event types (`approval.requested`, `transaction.sale_completed`, etc.) remain during migration; `notification.created` drives inbox refresh. Legacy toast handlers stay until UI fully uses persisted notifications.

6. **Contracts** — Shared `ActivityKind` and notification DTO shapes in `@gaming-cafe/contracts`; OpenAPI source of truth per ADR-0004.

## Consequences

### Positive

- Staff and admins can review missed updates after reconnect
- Single activity timeline for operational audit
- Builds on ADR-0013 without replacing WS transport

### Negative

- Dual-write adds latency to hot paths (mitigated: same transaction where possible)
- Recipient resolution queries admin user list on each record

### Risks

| Risk | Mitigation |
|------|------------|
| Notification volume growth | Indexed queries; paginated APIs |
| Duplicate toasts during transition | `notification.created` toast optional; legacy handlers retained briefly |

## Alternatives Considered

### Query `realtime_outbox` as history

- Pros: No new tables
- Cons: Wrong retention model, no read/unread, not user-scoped
- **Rejected**

### Notifications only (no activity_log)

- Pros: Simpler schema
- Cons: Admin history requires aggregating all user inboxes
- **Rejected** — separate activity_log powers Log History page

## References

- [ADR-0013](0013-realtime-websocket-channel.md) — WebSocket outbox
- [ADR-0004](0004-shared-api-types-openapi.md) — OpenAPI types
