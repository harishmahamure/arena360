# M01 — Tenant and policy foundation

## Outcome

One backend can safely represent organizations, locations, zones, stations,
users, settings, and policies without cross-tenant access or ambiguous scope.

## In scope

- Organization/location identity, membership, location access, and request
  context.
- Clean organization/location ownership model without a compatibility tenant.
- Canonical Station plus capability/attached-device model.
- Typed organization/location settings with revisions and effective-value
  resolution.
- Versioned policy lifecycle: draft, validate, simulate, publish, schedule,
  supersede, and roll back.
- Tenant-scoped audit, cache keys, realtime topics, jobs, storage paths, metrics,
  and rate limits.
- Versioned import schemas and validation for organizations, locations,
  identities, Stations, settings, and policies; the separate migration tool
  performs legacy extraction and transformation.

## UX discovery

- Organization/location switcher and clear active-scope indicator.
- Access-denied versus not-found behavior.
- Setting inheritance, override, history, and rollback.
- Station type/capability configuration without PC-only assumptions.

## Non-goals

- No product-surface implementation beyond prototypes.
- No billing subscription, Player Self-Service, Edge, or Hub.

## Exit gate

Isolation tests prove no cross-tenant read/write/event/cache leakage; the new
Cloud and Station contracts work together; rollback and recovery are rehearsed
on representative new-system data.
