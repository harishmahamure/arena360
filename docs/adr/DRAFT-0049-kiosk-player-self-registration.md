# DRAFT-0049: Kiosk player self-registration

**Status**: Proposed
**Date**: 2026-06-26
**Deciders**: Platform team (gaming-cafe kiosk working group)

**Extends**: [ADR-0017](0017-kiosk-player-device-auth.md) (Accepted)

## Context

Players today are created only by staff or admin via `POST /auth/register`
(`AdminOrStaff` guard). Kiosk v1 covers player login ([US-KAUTH-001](
../REQUIREMENTS-KIOSK.md)) but not self-service account creation at the
station. Operators want new customers to register without counter staff typing
credentials for them.

## Decision

Introduce **`POST /auth/register/player`**, authenticated with the **device JWT**
(same trust boundary as `POST /auth/login/player`):

- Only registered, operational kiosks may call the endpoint.
- Server **forces** `role = player`; no staff/admin escalation.
- `createdBy` / `updatedBy` are `NULL` (self-registration).
- Per-device rate limiting prevents abuse from a compromised station token.
- Structured error codes with optional `details.field` for inline form errors
  (e.g. `AUTH_USERNAME_ALREADY_EXISTS` → username field).

**Unchanged:** `POST /auth/register` remains `AdminOrStaff` only.

### Post-registration UX

Kiosk shows a success screen instructing the player to ask staff for plan time,
then returns to the login screen. No auto-login (login is still gated by
US-KAUTH-003 plan eligibility).

## Consequences

### Positive

- Self-service onboarding at the station without staff credentials.
- Consistent with ADR-0017 dual-header device auth model.
- Field-level errors improve kiosk form UX.

### Negative

- Additional abuse surface on physical kiosks (mitigated by device JWT + rate limit).
- Two registration HTTP paths to maintain (admin vs kiosk).

### Risks

| Risk | Mitigation |
|------|------------|
| Spam accounts from stolen device JWT | Per-device rate limit; admin device revoke |
| Duplicate phone numbers | No uniqueness constraint in v1 (same as admin register) |

## Alternatives Considered

### Open `/auth/register` to device JWT

Rejected: would require role-guard branching in one handler and blur admin audit semantics.

### Fully public registration (no auth)

Rejected: enables internet-wide account farming; device JWT is sufficient for physical stations.

## Implementation Notes

- New ErrorCodes: `AUTH_USERNAME_ALREADY_EXISTS`, `REGISTRATION_RATE_LIMITED`
- Kiosk phases: `create-account`, `create-account-success`
- Shared yup schema in `@gaming-cafe/utils`

## References

- [REQUIREMENTS-KIOSK.md](../REQUIREMENTS-KIOSK.md) — US-KAUTH-007 (new)
- [ADR-0017](0017-kiosk-player-device-auth.md)
