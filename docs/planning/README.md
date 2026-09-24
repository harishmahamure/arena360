# Arena360 product planning

This directory translates [the product vision](../PRODUCT_VISION.md) into a
delivery scaffold. It is planning material, not an implementation commitment.
No product code is authorized by these documents.

## Fixed boundary

Only these existing applications are implementation input candidates:

- `apps/backend`: reuse proven domain and infrastructure internals where they
  suit the new cloud/control-plane architecture.
- `apps/kiosk`: reuse proven Windows enforcement and launcher internals where
  they suit the new Station/Agent architecture.

There is no backward-compatibility requirement. Existing APIs, database
schemas, events, routes, workflows, and UI contracts may be replaced to create
one coherent system. Existing-data migration will be handled by a separate
user-owned migration tool and does not constrain the new product model.

The complete application family is planned as Arena360 Admin, Arena360 Owner,
Arena360 Staff, Arena360 Player Self-Service, and the Arena360 Marketing
Website, alongside the backend, kiosk/Station, Edge, Hub, and Intelligence
products. The existing `apps/admin` application and shared frontend packages
are reference material only.

## Planning set

| File | Purpose |
| --- | --- |
| [`planning/global.yml`](../../planning/global.yml) | Machine-readable global assumptions, boundaries, and quality gates |
| [`planning/milestones.yml`](../../planning/milestones.yml) | Ordered milestone registry and dependencies |
| [Product surfaces](PRODUCT_SURFACES.md) | Responsibilities and boundaries for the complete Arena360 application family |
| [Data migration boundary](DATA_MIGRATION_BOUNDARY.md) | Contract between the new system and the separately owned migration tool |
| [Capability baseline](CAPABILITY_BASELINE.md) | What is reusable, adaptable, reference-only, or missing |
| [UX plan](UX_PLAN.md) | Personas, information architecture, critical journeys, and UX gates |
| [Delivery management](DELIVERY_MANAGEMENT.md) | Work states, acceptance rules, risks, and governance |
| [Decision log](DECISIONS.md) | Product and architecture decisions that affect more than one milestone |
| [Milestones](milestones/) | Small outcome-based plans with entry and exit gates |
| [Milestone template](MILESTONE_TEMPLATE.md) | Required shape for future milestone plans |

## Reading order

1. Read the vision and the capability baseline.
2. Confirm the reuse boundary and open decisions.
3. Approve M00 before treating later milestones as ready.
4. Advance a milestone only when its entry gate is satisfied.
5. Update the baseline and decision log when source behavior or scope changes.

## Status vocabulary

- `proposed`: outcome and boundary exist, but discovery is incomplete.
- `discovery`: contracts, UX, risks, and dependencies are being resolved.
- `ready`: acceptance criteria and dependencies are approved.
- `active`: implementation is authorized and in progress.
- `validating`: the slice is complete and is being proven against exit gates.
- `done`: exit gates are met and evidence is linked.
- `paused`: intentionally stopped, with a documented restart condition.
