# ADR-0018: Kiosk WebSocket ACL for device tokens

**Status**: Accepted
**Date**: 2026-05-30
**Deciders**: Platform team (gaming-cafe kiosk working group)

**Amends**: [ADR-0013](0013-realtime-websocket-channel.md) (ACL table and event payload schemas only)
**Depends on**: [ADR-0017](0017-kiosk-player-device-auth.md) (Accepted)

## Context

[ADR-0013](0013-realtime-websocket-channel.md) defines the WebSocket subsystem with
role-gated channels. The `device:{deviceId}` channel is currently restricted to
admin and staff subscribers ([`acl.rs`](../../apps/backend/src/realtime/acl.rs)
lines 39–46).

The kiosk must receive real-time events on its own device channel for:

- Session lifecycle (`session.started`, `session.ended`) — US-KSESSION-009
- Staff recharge while a player is active (`balance.updated`) — US-KSESSION-007
- Remote device status changes (`device.status_changed`) — US-KREG-006

[ADR-0017](0017-kiosk-player-device-auth.md) establishes that the kiosk connects
to `GET /realtime` with a **device JWT** via
`Sec-WebSocket-Protocol: bearer, <device-jwt>`. Player JWTs are used for HTTP
only in v1; the kiosk does not open a separate WS connection with a player token.

Existing outbox payloads from [`session_service.rs`](../../apps/backend/src/services/session_service.rs)
are minimal (`session_id`, `device_id` snake_case). Kiosk UI needs richer fields
for countdown refresh and toasts. This ADR standardizes payloads (resolves **OQ-2**).

Transport, outbox pattern, wire protocol, and retention from ADR-0013 are **unchanged**.

## Decision

### Updated channel ACL

Amend ADR-0013 channel taxonomy. **Add** the following rows; do not remove
existing admin/staff rules.

| Token | Channel | Subscribe | Publish |
|-------|---------|-----------|---------|
| `device` JWT | `device:{ownDeviceId}` | **yes** — `ownDeviceId` must match JWT `userId` / `deviceId` | no (system only) |
| `device` JWT | `user:{playerId}` | **yes** — only if player has open `usage_sessions` row on this device | no |
| `device` JWT | `admin`, `staff`, `public`, `room:*` | no | no |
| `player` JWT | any WS channel | **not used in v1** | no |
| `admin` / `staff` | `device:{anyId}` | unchanged (yes) | unchanged (system only) |

**`user:{playerId}` subscribe validation** (K1 `be-ws-device-acl`):

On `Subscribe` to `user:{playerId}`, the handler queries:

```sql
SELECT 1 FROM usage_sessions s
WHERE s."deviceId" = $device_id
  AND s."endTime" IS NULL
  AND s."balanceId" IN (
    SELECT id FROM player_plan_balances WHERE "playerId" = $player_id
  )
LIMIT 1
```

Reject with `FORBIDDEN_CHANNEL` if no matching open session.

Device tokens **cannot** subscribe to another device's channel or another user's
channel without an active session on this device.

### Kiosk connection contract

| Setting | Value |
|---------|-------|
| Endpoint | `GET /realtime` |
| Auth header | `Sec-WebSocket-Protocol: bearer, <device-jwt>` |
| Initial subscribe | `["device:{ownDeviceId}"]` |
| After player login | Add `user:{playerId}` when session active |
| On session end / logout | Unsubscribe `user:{playerId}` |

Reconnect: exponential backoff ≤ 5 s (NFR); replay via existing `last_ack_id` / delivery table per ADR-0013.

### Standardized event schemas

All payloads use **camelCase** JSON keys in the `Event.payload` field.
Implementers may accept legacy snake_case during a transition window in K1 only.

#### `session.started` (channel: `device:{deviceId}`, `staff`)

```json
{
  "sessionId": "550e8400-e29b-41d4-a716-446655440000",
  "deviceId": "660e8400-e29b-41d4-a716-446655440001",
  "playerId": "770e8400-e29b-41d4-a716-446655440002",
  "balanceId": "880e8400-e29b-41d4-a716-446655440003",
  "startTime": "2026-05-30T14:00:00Z"
}
```

#### `session.ended` (channel: `device:{deviceId}`, `staff`)

```json
{
  "sessionId": "550e8400-e29b-41d4-a716-446655440000",
  "deviceId": "660e8400-e29b-41d4-a716-446655440001",
  "playerId": "770e8400-e29b-41d4-a716-446655440002",
  "reason": "voluntary",
  "endTime": "2026-05-30T16:30:00Z",
  "remainingMinutes": 12.5
}
```

`reason` enum (v1): `voluntary` | `ENDED_AUTO` | `force` | `offline_reconcile`

#### `balance.updated` (channel: `device:{deviceId}`, optionally `user:{playerId}`)

Published when staff completes a recharge affecting a player who may have an
active session on a kiosk (`be-recharge-events`).

```json
{
  "balanceId": "880e8400-e29b-41d4-a716-446655440003",
  "playerId": "770e8400-e29b-41d4-a716-446655440002",
  "deviceId": "660e8400-e29b-41d4-a716-446655440001",
  "remainingMinutes": 45.0,
  "sessionId": "550e8400-e29b-41d4-a716-446655440000"
}
```

`sessionId` is omitted when the player has no open session. Kiosk shows
"Time added" toast when `sessionId` matches the active session.

#### `device.status_changed` (channel: `device:{deviceId}`, `staff`)

```json
{
  "deviceId": "660e8400-e29b-41d4-a716-446655440001",
  "status": "under_maintenance",
  "previousStatus": "available"
}
```

### ACL implementation notes

Extend [`can_subscribe`](../../apps/backend/src/realtime/acl.rs):

```rust
// Pseudocode — K1
ChannelId::Device(id) => {
  if claims.is_admin_or_staff() { Ok(()) }
  else if claims.roles.contains("device") && claims.device_id() == Some(*id) { Ok(()) }
  else { Err(forbidden) }
}
```

Add `JwtUserClaims::is_device()` and `device_id()` helpers mirroring admin/staff checks.

## Consequences

### Positive

- Kiosk receives recharge and force-end events without staff polling only.
- Payload schemas are explicit for `@gaming-cafe/api-types` documentation and kiosk UI.
- Admin/staff WS behavior unchanged.

### Negative

- `user:{playerId}` subscribe requires DB lookup on each subscribe (acceptable at cafe scale).
- Payload migration from snake_case requires updating `session_service` outbox calls in K1.

### Risks

| Risk | Mitigation |
|------|------------|
| Device token subscribes to wrong channel | JWT `deviceId` must match channel suffix |
| Stale `user:` subscription after session end | Kiosk unsubscribes on logout; server rejects if session closed |
| Event schema drift | Document here; version in OpenAPI realtime appendix if added later |

## Alternatives considered

### A. Separate kiosk-only WS endpoint

- Pros: Simpler ACL.
- Cons: Duplicates ADR-0013 transport; two realtime paths to maintain.
- **Rejected.**

### B. Player JWT for WebSocket

- Pros: Natural `user:` channel ACL.
- Cons: Player token expires mid-session; device identity still needed for `device:` channel.
- **Rejected for v1** — device JWT only per ADR-0017.

## Out of scope

- Postgres migration changes
- New external crates
- Replacing outbox / dispatcher (ADR-0013)

## Implementation notes

After **Acceptance**:

1. `be-ws-device-acl` — extend `acl.rs` + subscribe handler validation
2. `be-recharge-events` — publish `balance.updated` per schema above
3. Update `session_service` outbox payloads to camelCase schemas
4. `kiosk-ws-client` — subscribe/unsubscribe per connection contract

## References

- [ADR-0013](0013-realtime-websocket-channel.md)
- [ADR-0017](0017-kiosk-player-device-auth.md)
- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) — US-KSESSION-007, US-KREG-006
- [PLANNER-KIOSK.md](../PLANNER-KIOSK.md) — `kiosk-adr-ws-acl`, `be-ws-device-acl`
