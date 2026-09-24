# M02 — Shared UX system and Arena360 Admin

## Outcome

Arena360 has a new shared experience foundation and a separate internal Admin
product for safely operating the platform and its tenants.

## In scope

- New web foundation chosen without `apps/admin` compatibility constraints.
- Design tokens, typography, density, status language, forms, data tables,
  dialogs, command/search patterns, and notifications.
- Shared identity handoff, tenant/location context, error conventions,
  accessibility rules, telemetry, and safe cross-product navigation.
- Arena360 Admin shell, privileged authentication, MFA, authorization,
  organization/location lifecycle, subscription/entitlement view, deployment
  health, quotas/abuse signals, global audit, and assisted-support access.
- Explicit stale, offline, failure, and privilege-escalation states.

## Validation journeys

- Platform admin creates or suspends an organization.
- Support diagnoses a tenant issue using approved, time-bound access.
- Operations reviews deployment health and quota warnings.
- A privileged action captures reason, scope, and audit evidence.

## Non-goals

- No reuse or wholesale migration of old admin pages.
- No Staff venue-operation workflows, Owner analytics, or Player journeys.

## Exit gate

The shared UX foundation passes accessibility review and Arena360 Admin proves
tenant lifecycle plus audited support access against the new system contracts.

