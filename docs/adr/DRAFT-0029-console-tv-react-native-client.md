# DRAFT-0029: Console TV React Native client

**Status**: Proposed
**Date**: 2026-06-05
**Deciders**: Platform team

**Implements**: Console TV overlay for PlayStation stations
**Supersedes**: _(none — complements ADR-0016/0017)_

## Context

PlayStation stations use a TV for display but do not run a full PC kiosk. Staff assign sessions from the admin counter; the TV shows a countdown overlay only — no player login, no game launcher, no Windows lockdown.

Android TV smart TVs and Amazon Fire TV sticks both run Android. A React Native Android build targets both with leanback launcher configuration and sideload deployment.

## Decision

Add `apps/console-tv`, a React Native 0.76+ Android app that:

1. Provisions once via admin OTP + existing `POST /devices/provision` (device types `PS5`, `PS4`, `CONSOLE`).
2. Subscribes to `device:{id}` WebSocket channel with device JWT only.
3. Displays a HUD countdown when a staff-started session is active, driven by **WebSocket events and local tick** (no REST session polling on the TV).
4. Anchors the countdown from `remainingMinutes` on `session.started` when present; falls back to a “Session active” indicator if the payload lacks time fields.
5. Clears the overlay on `session.ended` WebSocket events.
6. Plays audio reminders at 10/5/2 minutes via a native Android module.

Screen states: `setup` (first boot), `idle` (no session), `overlay` (active session HUD).

**No new backend REST routes.** Staff continue using existing `POST /sessions` and `PATCH /sessions/{id}/end`. Device registration uses existing `POST /devices/provision`.

## Consequences

### Positive

- Reuses monorepo packages (`contracts`, optional `utils` patterns).
- Same deployment model as kiosk provisioning (admin on device).
- One APK for Android TV and Fire TV.
- Zero backend changes required for MVP (WebSocket lifecycle only).

### Negative

- Accurate countdown digits require `remainingMinutes` on the `session.started` WebSocket payload to `device:{id}`; committed backend currently sends only `sessionId` and `deviceId` — enrichment is a separate backend concern if needed.
- New RN workspace member; separate release pipeline from Tauri kiosk.
- Overlay behavior varies by TV OEM.

### Risks

- Fire OS API restrictions → Mitigation: test on Firestick; use `ANDROID_ID` in fingerprint placeholders.
- No native PS control → Mitigation: time management only; staff ends sessions from admin.

## Alternatives Considered

### Device-scoped REST session API (`/kiosk/console/sessions/*`)

- Pros: authoritative poll/heartbeat without player token.
- Cons: new backend surface; rejected for this MVP in favour of WebSocket + local countdown.

### Tauri Android

- Cons: immature for TV overlay; rejected.

## References

- ADR-0016 (kiosk monorepo)
- ADR-0017 (device JWT auth)
- ADR-0018 (WebSocket device ACL)
