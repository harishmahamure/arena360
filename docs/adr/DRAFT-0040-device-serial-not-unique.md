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
2. **Reprovision** only when the stored fingerprint **MAC** matches the presented
   MAC (case-insensitive). BIOS serial and UUID are not used for lookup.
3. **Persist MAC** in `devices."serialNumber"` at provision time (admin-visible
   station identifier). OEM BIOS serial strings are ignored.

## Consequences

### Positive

- Factory reset on the same PC reprovisions when MAC is unchanged.
- Admin device list shows MAC in the serial number field.

### Negative

- Serial field in admin no longer reflects BIOS serial.
- NIC replacement changes MAC → registers as a new device row.

## Alternatives Considered

### Drop unique index only

Rejected: app logic still routed different PCs through serial lookup and failed
on fingerprint mismatch.

### Require unique serial per device

Rejected: fails on real-world OEM hardware.

## References

- `apps/backend/src/services/device_service.rs` — `provision()`
- `apps/backend/migrations/*_drop_devices_serial_unique.*`
