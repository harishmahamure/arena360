# DRAFT-0037: Console TV kiosk-style provisioning

**Status**: Proposed
**Date**: 2026-06-14
**Deciders**: Platform team
**Supersedes**: [DRAFT-0035](DRAFT-0035-android-tv-console-station.md) (provisioning sections only)

## Context

[DRAFT-0035](DRAFT-0035-android-tv-console-station.md) chose admin SSO over WebSocket (`device-pairing`, `tv_provision`, `sso.token.created`) to avoid keyboard entry on Android TV remotes. Operators now use touch-capable TV hardware and prefer the same **admin-on-device** registration flow as the PC kiosk ([DRAFT-0023](DRAFT-0023-admin-authorized-device-registration.md)).

Session lifecycle, TV session APIs, local clock, CEC, and admin-started gameplay remain as defined in DRAFT-0035.

## Decision

1. **Provisioning** on Console TV mirrors the PC kiosk:
   - `POST /auth/login/admin` on the device (username, password, optional TOTP)
   - `POST /devices/provision` with admin bearer + `provisionClient: "console-tv"`
   - No pre-created device UUID required; provision creates the PS station row
2. **Deprecate from Console TV client and admin UI** (endpoints remain for backward compatibility):
   - `POST /auth/device-pairing`
   - `POST /auth/sso/tokens` with `purpose: tv_provision`
   - `gamingcafe://tv/sso` deep link
   - Admin **Send TV login** button
3. **Unchanged**: `GET /tv/sessions/current`, `PATCH /tv/sessions/{id}/end`, WS `session.started` / `balance.updated` / `session.ended`, no player login on TV.

## Consequences

### Positive
- One registration mental model for staff across PC kiosk and Console TV
- No admin ↔ TV coordination during setup (device ID + SSO push)
- Reuses existing DRAFT-0023 backend; no new HTTP surface

### Negative
- Admin credentials entered on the TV (mitigated: short-lived in-memory admin token, same as kiosk)
- Touch keyboard required on registration screens

### Risks
- Shared admin credentials on shared TV → same as kiosk; use per-site admin accounts with TOTP

## Alternatives Considered

### Keep DRAFT-0035 SSO-over-WS provisioning
- Pros: no credential entry on TV. Cons: complex two-screen flow; rejected per operator feedback.

## References

- [DRAFT-0023](DRAFT-0023-admin-authorized-device-registration.md)
- [DRAFT-0035](DRAFT-0035-android-tv-console-station.md)
- [REQUIREMENTS-CONSOLE-TV.md](../REQUIREMENTS-CONSOLE-TV.md)
