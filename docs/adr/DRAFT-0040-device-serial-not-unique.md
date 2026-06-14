# DRAFT-0040: Device serial not unique; fingerprint-keyed reprovision

**Status**: Proposed
**Date**: 2026-06-17
**Deciders**: Platform team

**Relates to**: [DRAFT-0023](DRAFT-0023-admin-authorized-device-registration.md), [ADR-0017](0017-kiosk-player-device-auth.md)

## Context

Kiosk provisioning (`POST /devices/provision`) looked up existing devices by
`serialNumber` (BIOS serial from fingerprint). Many OEM PCs share placeholder
serials (`To be filled by O.E.M.`, `Default string`, etc.), so a second physical
station hit:

> Serial number is already registered to device '…'

even when MAC and BIOS UUID differed. A TypeORM-era **unique constraint** on
`devices."serialNumber"` could also block INSERT.

## Decision

1. **Drop** the DB unique constraint/index on `devices."serialNumber"`.
2. **Reprovision** only when a registered device's stored fingerprint matches the
   presented one with drift ≤ 1 (ADR-0017), keyed by **BIOS UUID** (or MAC when
   BIOS UUID is a known placeholder).
3. **Stop** using serial number as the reprovision lookup key; serial remains
   informational metadata on the device row.

## Consequences

### Positive

- Multiple stations can register with duplicate OEM serial strings.
- Factory reset / NIC change on the same PC still reprovisions the same row.

### Negative

- Serial alone is no longer a reliable hardware identifier in admin.
- Duplicate placeholder BIOS UUIDs (rare) could still collide; MAC fallback
  mitigates the all-zero placeholder case.

## Alternatives Considered

### Drop unique index only

Rejected: app logic still routed different PCs through serial lookup and failed
on fingerprint mismatch.

### Require unique serial per device

Rejected: fails on real-world OEM hardware.

## References

- `apps/backend/src/services/device_service.rs` — `provision()`
- `apps/backend/migrations/*_drop_devices_serial_unique.*`
