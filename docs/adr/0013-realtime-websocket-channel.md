# ADR-0013: Realtime WebSocket channel replaces SSE

**Status**: Accepted
**Date**: 2026-05-28
**Deciders**: Platform team
**Supersedes**: ADR-0009 (real-time portion only — the rest of ADR-0009 remains accepted)

## Context

ADR-0009 introduced SSE via `tokio::sync::broadcast` as the real-time
channel. The implementation (`apps/backend/src/sse/`) provides four
event types (`session.started`, `session.ended`, `device.status_changed`,
`transaction.created`) through a single `GET /sse?topics=…` endpoint.

Current limitations:

1. **Best-effort delivery only.** The broadcast channel is in-memory
   with capacity 100. Slow or disconnected subscribers silently lose
   events. There is no replay mechanism.
2. **Unidirectional.** SSE is server→client. Clients cannot subscribe
   to specific channels, send ACKs, or publish chat messages without a
   separate REST call.
3. **No role-gated channels.** The SSE handler streams all matching
   topics to every authenticated user. There is no concept of an
   admin-only or user-scoped channel.
4. **No consumer in production.** The admin panel does not connect to
   SSE; it uses polling (60s sessions, 30s detail pages). The SSE
   module is effectively dead code.

New requirements from the admin panel and operational workflow:

- Staff product sales must notify admins in real time.
- Cash-deposit and expense approval requests must push to admin.
- Approval decisions must push back to the requesting staff member.
- Support for admin-managed named rooms (e.g. ad-hoc support threads).
- At-least-once delivery across client reconnects and server restarts.
- Chat messages between users within channels they belong to.

## Decision

Replace the SSE module with a **WebSocket subsystem** built on
`axum::extract::ws`, using a **Postgres-backed transactional outbox**
for durable at-least-once delivery.

### Transport

- **Protocol**: WebSocket via `axum::extract::ws` (feature `ws` on
  axum 0.8).
- **Auth**: JWT passed via `Sec-WebSocket-Protocol: bearer, <token>`.
  No tokens in query strings (avoids access-log leaks). Validated
  before upgrade using the existing `decode_token()` from
  `apps/backend/src/middleware/auth.rs`.
- **Endpoint**: `GET /realtime` — returns 101 Upgrade on valid JWT.

### Channel taxonomy and ACL

| Channel | Subscribe ACL | Publish ACL | Durable |
|---|---|---|---|
| `public` | any authenticated user | system only | no |
| `admin` | role `admin` | system + services | yes |
| `staff` | role `admin` or `staff` | system + services | yes |
| `user:{userId}` | self, or role `admin` | system + services + other users (chat) | yes |
| `device:{deviceId}` | role `admin` or `staff` | system + services | no |
| `room:{name}` | listed in `realtime_room_members` | room members + system | yes |

### Wire protocol (JSON text frames)

Client → Server:

```json
{ "type": "Subscribe", "channels": ["admin", "user:abc-123"] }
{ "type": "Unsubscribe", "channels": ["admin"] }
{ "type": "Ack", "msg_id": 42 }
{ "type": "Publish", "channel": "room:support-1", "payload": { "text": "hello" } }
{ "type": "Ping" }
```

Server → Client:

```json
{ "type": "Welcome", "user_id": "uuid", "roles": ["admin"] }
{ "type": "Subscribed", "channels": ["admin"] }
{ "type": "Event", "msg_id": 42, "channel": "admin", "event_type": "transaction.sale_completed", "payload": {}, "ts": "..." }
{ "type": "Error", "code": "FORBIDDEN_CHANNEL", "message": "..." }
{ "type": "Pong" }
```

### Durable outbox pattern

Services publish events by inserting into `realtime_outbox` within the
same SQLx transaction as the business write. A Postgres `AFTER INSERT`
trigger fires `pg_notify('realtime_outbox_new', id)`. A singleton
dispatcher task (`tokio::spawn`) listening via `sqlx::postgres::PgListener`
wakes, reads the outbox row, computes the audience, inserts per-subscriber
rows into `realtime_deliveries`, and pushes frames to connected clients.

On reconnect, clients send a `Subscribe` with their `last_ack_id`. The
dispatcher replays all unacked rows from `realtime_deliveries` for that
subscriber.

Retention: a periodic tokio task deletes acked rows older than 7 days
and unacked rows older than 30 days (configurable via env).

### Named rooms

Admin-managed via REST:

- `POST /realtime/rooms` — create (admin only)
- `GET /realtime/rooms` — list rooms the caller belongs to
- `POST /realtime/rooms/{id}/members` — add member (admin only)
- `DELETE /realtime/rooms/{id}/members/{userId}` — remove (admin only)

Membership stored in `realtime_room_members` and checked on subscribe
and publish.

### Service integration

| Service call site | Event published to |
|---|---|
| `SessionService::start_session` | `staff` + `device:{id}` |
| `SessionService::end_session` | `staff` + `device:{id}` |
| `DeviceService::update_status` | `staff` + `device:{id}` |
| `TransactionService::create_product_purchase` (actor=staff) | `admin` — `transaction.sale_completed` |
| `CashDepositService::create` | `admin` — `approval.requested` |
| `CashDepositService::approve/reject` | `user:{created_by}` + `admin` |
| `ExpenseService::create` (pending) | `admin` — `approval.requested` |
| `ExpenseService::approve/reject` | `user:{created_by}` + `admin` |

## Consequences

### Positive

- **At-least-once delivery** — events survive reconnects and restarts.
- **Role-gated channels** — admins, staff, and individual users see
  only what they should.
- **Bidirectional** — subscribe/ack/chat without REST round-trips.
- **Single real-time path** — no SSE + WS duality to maintain.
- **Named rooms** — extensible for support threads, shift handover.
- **No new external deps** — axum `ws` feature + existing sqlx/tokio.

### Negative

- **Postgres connection budget** — `PgListener` holds one long-lived
  connection per process.
- **Single-instance dispatcher** — horizontal scaling across pods
  requires a future pub/sub layer (Redis, NATS, or Postgres logical
  replication). Documented as out of scope.
- **Outbox table growth** — mitigated by retention task and partial
  index on unacked deliveries.
- **Browser auth quirk** — `Sec-WebSocket-Protocol` header is the only
  standard way to pass auth on WS upgrade without query params. The
  admin client wraps this; app code never sees it.

### Risks

- **Risk**: Outbox writes add latency to business transactions.
  **Mitigation**: Single additional INSERT in the same transaction;
  `pg_notify` costs microseconds.
- **Risk**: `PgListener` disconnects silently.
  **Mitigation**: sqlx `PgListener` auto-reconnects; dispatcher polls
  for missed IDs on reconnect.
- **Risk**: Large replay sets on long-offline subscribers.
  **Mitigation**: 30-day retention limit; client-side cursor prevents
  unbounded replay.

## Alternatives considered

### A. Keep SSE, add in-memory ring buffer

- Pros: No schema changes, simple.
- Cons: Still unidirectional, no durable delivery, no role gating,
  loses events on server restart.
- **Why rejected**: Does not meet at-least-once or admin-channel
  requirements.

### B. Add WebSocket with in-memory ring buffer (no outbox)

- Pros: Bidirectional, role-gated channels, simpler than outbox.
- Cons: Events lost on server restart; no replay after long disconnect.
- **Why rejected**: "At-least-once" requirement explicitly includes
  surviving restarts.

### C. External broker (Redis Streams / NATS)

- Pros: Horizontally scalable, battle-tested pub/sub.
- Cons: New infrastructure dependency (changes deployment shape —
  requires its own ADR); overkill for single-instance deployment.
- **Why rejected**: Premature for single-node deployment. Postgres
  outbox + `pg_notify` provides the durability guarantee without
  adding operational surface. Can adopt Redis pub/sub later for
  cross-pod fanout without changing the outbox (Postgres remains
  source of truth).

### D. Keep SSE alongside WebSocket

- Pros: Less churn — SSE continues for existing topics.
- Cons: Two real-time paths to maintain, test, and document.
  Admin has no SSE consumer today, so migration cost is near zero.
- **Why rejected**: Single path is simpler long-term; admin-side
  migration cost is negligible since no SSE consumer exists.

## Out of scope

- Kiosk WS auth (device tokens) — kiosk app not yet in monorepo.
- Horizontal dispatcher scaling across pods.
- Read-receipts beyond delivery ACK.
- Message editing/deletion for chat.
- Per-frame rate limiting.

## Implementation notes

- New module: `apps/backend/src/realtime/` (channel, acl, frame,
  connection, dispatcher, outbox, deliveries, rooms, handler).
- New migration: `realtime_outbox`, `realtime_deliveries`,
  `realtime_rooms`, `realtime_room_members` tables with `pg_notify`
  trigger.
- Existing `EventService` and `apps/backend/src/sse/` are deleted.
- Admin panel adds `RealtimeProvider` + `useRealtime()` hook.
- OpenAPI regenerated for room endpoints; `pnpm gen:api-types` run.

## References

- ADR-0009: Rust Axum backend (this ADR supersedes the real-time
  portion)
- ADR-0008: Runtime contracts package (ErrorCode enum)
- `apps/backend/src/sse/` — current SSE module being replaced
- `apps/backend/src/middleware/auth.rs` — JWT validation reused
