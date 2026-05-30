# DRAFT-0023: Admin-authorized device registration

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team
**Supersedes**: ADR-0017 (registration-code pairing portion)

## Context

First-time kiosk registration today uses a one-time **registration code** issued
from the admin portal and redeemed at the public `POST /devices/register`
endpoint (columns added in migration `20260530000004`). The redesigned flow
mirrors the ggLeap installer: on a fresh device the operator performs an **admin
login on the device**, then names the PC; the admin session itself authorizes
registration. There is no separate code to copy between two screens.

## Decision

1. **Admin login authorizes registration.** On a device with no device token, the
   kiosk shows an admin login (username/password → OTP, reusing
   `/auth/login/admin` + `/auth/verify-otp`). The returned admin access token is
   held in memory for the next step only.
2. **New endpoint** `POST /devices/provision` (admin JWT required):
   body `{ fingerprint, name, deviceType, deviceSubType, location, serialNumber? }`.
   It creates (or updates) the device row as `registered`, stores the fingerprint
   snapshot, and returns `{ accessToken (device token), device }` — the same shape
   as the old register response.
3. **Retire the public code flow.** `POST /devices/register` and
   `POST /devices/{id}/registration-code` are deprecated; the registration-code
   columns from `20260530000004` are dropped (down migration restores them).
4. **Fingerprint drift policy (ADR-0017) is unchanged** for player login.

## Consequences

### Positive
- One coherent on-device install flow; no code transcription between screens.
- Registration is gated by real admin credentials + OTP.

### Negative
- A privileged admin must be physically present to provision a device.
- Reverses the just-added registration-code columns.

### Risks
- **Admin credentials entered on a kiosk.** **Mitigation**: lockdown stays
  `Locked` during admin entry (ADR-0020); the admin token is short-lived and only
  used to call `/devices/provision`.

## Alternatives Considered

### Keep code, gate behind admin login
- Pros: smaller diff. Cons: two-screen transcription remains; product wants the
  ggLeap single-flow. Rejected.

## References
- ADR-0017 Kiosk player and device JWT authentication (amended)
- ADR-0020 Kiosk Windows lockdown
