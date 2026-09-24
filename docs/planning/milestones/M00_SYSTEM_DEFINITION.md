# M00 — System definition and replacement boundaries

## Outcome

The team has one approved definition of the complete Arena360 system, its
product surfaces, shared domain language, replacement boundaries, and the
existing backend/kiosk internals worth carrying forward.

## In scope

- Define Arena360 Admin, Owner, Staff, Player Self-Service, Marketing Website,
  Cloud, Station/Agent, Edge, Hub, and Intelligence responsibilities.
- Inventory backend and kiosk capabilities as reuse candidates, not contracts.
- Define new cross-product states for Station, session, money, entitlement,
  reservation, order, automation, identity, and audit.
- Classify current and uncommitted work as reuse, redesign, or discard.
- Confirm that legacy API, schema, event, route, workflow, UI, and data
  compatibility is not required.
- Establish reliability, security, accessibility, observability, and support
  targets for the new system.

## Deliverables

- Approved product-surface map and ownership boundaries.
- New-system domain glossary and state-machine inventory.
- Reuse/extraction map for backend and kiosk internals.
- Initial API/event principles without legacy-shape constraints.
- Selected import-boundary approach and identifier rules for the separate
  user-owned migration tool.
- Accepted PD-001 through PD-005 decisions, or documented replacements.

## Non-goals

- No product implementation or migration-tool implementation.
- No promise to preserve an existing public or internal contract.

## Exit gate

Every product has an owner and boundary, shared concepts have one proposed
definition, and no unresolved assumption can force later milestones back into
the legacy system shape.
