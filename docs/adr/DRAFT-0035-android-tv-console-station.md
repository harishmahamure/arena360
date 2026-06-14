# DRAFT-0035: Android TV console station (PlayStation)

**Status**: Proposed
**Date**: 2026-06-11
**Deciders**: Platform team
**Amends**: [ADR-0017](0017-kiosk-player-device-auth.md) (device client scope), [ADR-0018](0018-kiosk-ws-device-acl.md) (`sso.token.created` event, pairing JWT)

## Context

PlayStation stations (`PS5`, `PS4`) need a device agent on Android TV: event-driven sessions via WebSocket, local interpolated session clock, HDMI-CEC to the console, and staff SSO provisioning without TV keyboard entry. PC stations remain on the Windows Tauri kiosk ([ADR-0002](0002-kiosk-tauri-canonical.md)).

## Decision

1. **New app** `apps/console-tv` (Kotlin Android TV, not in pnpm workspace).
2. **Device APIs** `GET /tv/sessions/current`, `PATCH /tv/sessions/{id}/end` — device JWT only; PlayStation device types only.
3. **SSO** `POST /auth/sso/tokens` (admin/staff), `POST /auth/sso/redeem` (public fallback), `POST /auth/device-pairing` (pre-provision WS).
4. **WebSocket** `sso.token.created` on `device:{deviceId}` when admin generates SSO.
5. **Pairing JWT** role `device_pairing` — 5 min TTL; subscribe only to own `device:{id}` before device JWT exists.
6. **Session sync** WS events + local clock; auto-end is the only device HTTP call during a session.

## Consequences

### Positive
- PlayStation stations get a fit-for-purpose TV agent without player login on device.
- SSO over WS removes QR/token entry on the remote.

### Negative
- New platform surface alongside Tauri kiosk; separate release pipeline.

### Risks
- CEC varies by TV brand → degraded manual mode.
- Pairing JWT is public → short TTL + device must pre-exist in admin.

## References

- Plan: Android TV Console Station requirements
- [DRAFT-0023](DRAFT-0023-admin-authorized-device-registration.md)
