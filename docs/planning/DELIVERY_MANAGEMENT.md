# Delivery and product management

This plan uses small, outcome-based milestones. A milestone is complete only
when a usable vertical slice and its operational evidence meet the exit gate;
finishing a list of tickets is not enough.

## Work hierarchy

```text
Vision
└── Milestone (M00)
    ├── Outcome
    ├── Journey or capability slice
    ├── Discovery item
    ├── Delivery item
    ├── Validation item
    └── Decision or risk
```

Use IDs `Mxx` for milestones, `Mxx-Oyy` for outcomes, and `ADR-xxxx` for
cross-cutting decisions. Every delivery item should link one outcome and one
acceptance scenario.

## Ready gate

A milestone can move to `ready` when:

- the user and business outcome is measurable;
- scope and explicit non-goals are approved;
- API/event/data contract impact is known;
- critical UX states have been prototyped;
- privacy, security, tenant, offline, and abuse implications are reviewed;
- dependencies have owners and acceptance evidence;
- rollout, rollback, replacement, and support expectations are documented;
- any rollout using existing data has an approved import-contract version and
  coordination plan with the separately owned migration tool.

## Done gate

A milestone can move to `done` when:

- acceptance scenarios pass on the supported deployment modes;
- authorization and tenant-isolation tests pass;
- migrations and recovery paths have been rehearsed where applicable;
- audit, observability, alerts, and support runbooks exist;
- accessibility and representative-user validation pass;
- API schemas and product documentation match shipped behavior;
- rollout metrics show no unresolved release blocker.

## Product review cadence

| Review | Purpose | Output |
| --- | --- | --- |
| Weekly milestone review | Resolve scope, decisions, and blockers | Updated state, owners, risks, and next evidence |
| UX review | Validate journeys and state coverage | Prototype decision and unresolved scenarios |
| System contract review | Keep new Arena360 products coherent | Contract, replacement, and rollout decision |
| Operational readiness review | Prove deploy, observe, support, recover | Go/no-go evidence |
| Post-release review | Compare outcome to target | Keep, iterate, or roll back decision |

## Required management views

- Milestone board grouped by state, not team.
- Dependency view generated from `planning/milestones.yml`.
- Decision log for product-wide choices.
- Risk register with probability, impact, mitigation, owner, and trigger.
- Contract change log for backend, kiosk, Edge, and public consumers.
- Outcome dashboard with adoption, reliability, revenue, and support signals.

## Initial risk register

| Risk | Probability | Impact | Mitigation / decision gate |
| --- | --- | --- | --- |
| Existing data is not fully tenant-scoped | High | Critical | M01 inventory and isolation proof before any multi-location rollout |
| Independently built products drift into incompatible domain models | High | Critical | One new-system contract, state-model, and vocabulary review from M00 onward |
| Rebuilding all non-core surfaces expands scope | High | High | Thin staff-first slices; owner and customer surfaces start only after prerequisite gates |
| Kiosk offline behavior is mistaken for Edge | High | High | Separate authority, storage, and conflict model in M07 discovery |
| Hardware automation causes unsafe or wrong-device actions | Medium | Critical | Simulation, pairing proof, idempotency, timeout, and manual fallback in M08 |
| Unlimited tier is abused through record creation or automation load | High | High | Quotas, per-resource limits, telemetry, fair-use policy, and enforcement UX in M09 |
| In-flight pricing/configuration work is assumed to constrain the new design | Medium | High | Treat it as evidence only; accept or replace it through M01 design and validation |
| Intelligence creates untrusted recommendations | Medium | Medium | Explanations, confidence, backtesting, approval, and bounded changes in M10 |

## Replacement and scope-change rule

New work enters the active milestone only if it is necessary for its stated
exit gate. Otherwise it is assigned to a later milestone or recorded as an
open decision. Any change to the backend/kiosk reuse boundary requires an
entry in [the decision log](DECISIONS.md). No milestone must preserve a legacy
contract merely because it exists; the chosen replacement must instead be
coherent across all affected new products.

The user-owned migration tool is managed as a separate workstream. Product
milestones own target import contracts and validation, not legacy extraction or
transformation. See [the data migration boundary](DATA_MIGRATION_BOUNDARY.md).

## Fair-use and abuse management

“Unlimited” means no normal usage-based product charge within the documented
service policy; it does not mean unbounded technical consumption. Before an
unlimited tier launches, Arena360 must define:

- record-creation and request limits by resource and time window;
- protections for bulk exports, uploads, realtime connections, notifications,
  automation runs, and device enrollment;
- customer-visible current usage and actionable limit errors;
- warning, temporary restriction, appeal, and support flows;
- internal override and emergency controls with audit records;
- published fair-use and abuse policy language aligned with enforcement.
