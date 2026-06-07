# DRAFT-0024: Kiosk session heartbeat and staff end confirmation

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team

**Relates to**: [ADR-0017](0017-kiosk-player-device-auth.md), [ADR-0018](0018-kiosk-ws-device-acl.md), [ADR-0021](DRAFT-0021-session-end-reason.md)

## Context

Kiosk sessions currently return `remainingMinutes` from the linked player balance
when the session starts or when the kiosk polls `/kiosk/sessions/current`.
The kiosk also receives `balance.updated` websocket events when a recharge or
plan purchase affects a player with an open session.

However, active usage is still primarily deducted when the session ends. This
creates several product and correctness issues:

- A long-running kiosk session can display stale balance if it relies only on
  start-time math or final session close deduction.
- Recharges and plan purchases should update active kiosk screens immediately
  through websocket events, with polling as fallback.
- The kiosk should play bundled countdown notifications at 10, 5, and 2 minutes.
- Staff/admin session close from the admin panel must require staff TOTP before
  ending the player's session.
- When staff/admin ends a session, the kiosk player must be logged out
  immediately via realtime `session.ended`.

Adding a new kiosk heartbeat endpoint is new HTTP surface area, so it requires
an ADR before implementation.

## Decision

Add a player/device-authenticated heartbeat endpoint:

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| `PATCH` | `/kiosk/sessions/{id}/heartbeat` | Device + player JWT | Deduct newly elapsed active usage and return authoritative remaining time |

The kiosk calls the heartbeat endpoint every two minutes while a player session
is active. The backend computes elapsed usage from server time, not client time,
and deducts only the delta that has not already been charged.

### Authoritative remaining time

`remainingMinutes` returned to kiosk clients must come from the persisted
`player_plan_balances.remainingMinutes` after any heartbeat, recharge, plan
purchase, poll, or session end operation. UI countdowns may tick locally between
server updates, but server responses are authoritative.

Admin and kiosk share one display hook (`useSessionRemainingMinutes` in
`@gaming-cafe/utils`). See [session-time-clock.md](../session-time-clock.md).
Frontends must **not** end sessions when the local clock reaches zero; only the
backend (auto-end after heartbeat) or explicit user/staff actions close sessions.

### Deduction model

Use `usage_sessions.timeCreditsConsumed` as the total minutes already charged
for the open session.

On heartbeat:

1. Load the open session and validate it belongs to the authenticated player and
   current device.
2. Compute `elapsedMinutes = ceil((now - startTime) / 60s)`.
3. Compute `delta = max(elapsedMinutes - timeCreditsConsumed, 0)`.
4. Deduct `delta` from the linked balance only when `delta > 0`.
5. Update `usage_sessions.timeCreditsConsumed` to the new charged total.
6. Return `KioskSessionResponseDto` with balance-backed `remainingMinutes`.
7. Publish `balance.updated` to the active device/player channels.

On final session end (voluntary, auto, force, staff PATCH):

1. Compute weighted wallet minutes from server `end_time` via
   `charge_session_delta` (see DRAFT-0033 when dynamic deduction is enabled).
2. Deduct only the delta not yet charged (`total − timeCreditsConsumed`).
3. Persist `durationMinutes` as wall-clock elapsed duration.
4. Persist/return the final total `timeCreditsConsumed` (wallet minutes).
5. Publish `session.ended` with final `remainingMinutes`.

Clients must omit `timeCreditsConsumed` on end except `offline_reconcile`.

This prevents double deduction when a session has already been charged by
heartbeat ticks.

### Realtime and polling fallback

Websocket remains the primary update path:

- `balance.updated` refreshes active kiosk remaining time after recharge,
  purchase, or heartbeat deduction.
- `session.ended` logs the kiosk player out immediately after staff/admin end,
  auto-end, force-end, or player end.

If websocket is disconnected, kiosk continues polling
`GET /kiosk/sessions/current` and heartbeat remains the two-minute authoritative
deduction path.

### Staff/admin end confirmation

Admin-panel staff users must complete TOTP before closing an active player
session from the admin panel. The implementation should reuse the existing staff
TOTP authentication pattern rather than inventing a second credential flow.

The backend must not allow a staff-originated session close without the required
TOTP/session confirmation. Admin role behavior may remain privileged if the
existing authorization model treats admin as already elevated, but staff close
requires TOTP.

When staff/admin closes a session, the backend publishes `session.ended` to:

- `device:{deviceId}`
- `user:{playerId}` when available
- staff/admin channels as needed for dashboards

Kiosk clients clear the player session and return to login when they receive the
matching `session.ended` event.

### Bundled audio notifications

Kiosk uses bundled static assets from `apps/kiosk/public/`:

- `10 minutes.mp3`
- `5 minutes.mp3`
- `2 minutes.mp3`

The browser/Tauri frontend plays these when the local countdown crosses each
threshold. Audio failure is non-fatal and must not block session logic.

## Consequences

### Positive

- Active sessions deduct time continuously instead of only at close.
- Recharges and plan purchases update active kiosk screens quickly.
- Staff/admin session closure logs the kiosk player out immediately.
- Final session close is idempotent with heartbeat deductions.
- Audio warnings improve player experience near expiry.

### Negative

- Adds one kiosk HTTP endpoint and more session accounting logic.
- Requires careful service-layer tests to prevent double deduction.
- Staff TOTP close flow touches both admin UI and backend authorization.

### Risks

| Risk | Mitigation |
|------|------------|
| Double deduction on heartbeat plus session end | Use `timeCreditsConsumed` as the charged total and deduct only deltas |
| Client clock drift | Compute elapsed minutes only on the backend |
| Websocket missed event | Keep `/kiosk/sessions/current` polling fallback |
| Browser blocks audio autoplay | Play sounds only after user interaction/session start; treat failures as non-fatal |
| Staff closes session without TOTP | Backend validates staff close confirmation before ending session |

## Alternatives considered

### Deduct only on session end

- Pros: Existing simpler model.
- Cons: Stale balance during long sessions, weaker recharge/expiry behavior,
  and worse crash/offline recovery. Rejected.

### Client-side countdown only

- Pros: No backend changes.
- Cons: Not authoritative, easy to drift, cannot enforce billing. Rejected.

### Background worker deducts all sessions

- Pros: Centralized.
- Cons: More infrastructure and scheduler failure modes than needed for v1.
  Rejected in favor of kiosk heartbeat.

## Implementation notes

After acceptance:

1. Add `PATCH /kiosk/sessions/{id}/heartbeat` in `apps/backend/src/handlers/kiosk.rs`.
2. Add `SessionService::heartbeat_for_player` and repository support to update
   `timeCreditsConsumed` without closing the session.
3. Update `SessionService::end` to deduct only uncharged deltas.
4. Publish `balance.updated` after heartbeat and `session.ended` after staff/admin end.
5. Add kiosk bundled audio playback and two-minute heartbeat.
6. Add staff/admin session close TOTP flow in the admin panel.

## References

- [ADR-0017](0017-kiosk-player-device-auth.md)
- [ADR-0018](0018-kiosk-ws-device-acl.md)
- [ADR-0021](DRAFT-0021-session-end-reason.md)
