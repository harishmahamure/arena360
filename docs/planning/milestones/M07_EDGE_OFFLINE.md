# M07 — Edge and offline continuity

## Outcome

A venue can continue approved critical operations during an internet outage
and converge safely when cloud connectivity returns.

## In scope

- New Arena Edge service/appliance boundary and local administrative console.
- Enrollment, signed identity, local discovery, health, upgrade, backup, and
  recovery.
- Explicit cloud/Edge authority model per command and data type.
- Local session, device, pricing/entitlement snapshot, event queue, and sync.
- Conflict detection/resolution, idempotency, ordering, retention, and clock
  assumptions.
- Local gateway for kiosk and future Hub communication.
- Offline status UX in staff, owner, kiosk, and customer surfaces.

## Failure exercises

- Internet loss mid-session and mid-payment.
- Edge restart, full disk, expired authority, clock drift, duplicate events,
  prolonged outage, and reconnect with conflicting cloud changes.

## Non-goals

- No physical TV/console automation or generalized rules engine.
- No assumption that every cloud feature works offline.

## Exit gate

An outage drill proves the documented offline matrix, no accepted financial or
session event is lost, conflicts are visible and recoverable, and cloud/kiosk
the new Cloud/Station/Edge contracts remain coherent.
