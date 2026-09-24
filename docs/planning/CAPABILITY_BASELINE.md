# Capability baseline and reuse map

Last inspected: 2026-09-24.

This baseline was derived from the repository knowledge graph, source files,
tests, generated OpenAPI document, and [current system documentation](../SYSTEM.md).
The source remains authoritative. Uncommitted configuration and pricing-policy
work is listed as in flight and must not be treated as a shipped capability.

## Reuse decision without compatibility constraints

This is a capability inventory, not a preservation contract. The new system may
replace existing APIs, schemas, events, routes, workflows, and UI structures.
Reuse is decided at the implementation-unit level only when it improves the new
design.

| Asset | Disposition | Planning interpretation |
| --- | --- | --- |
| `apps/backend` | Selective reuse and redesign | Reuse sound domain/infrastructure internals, but redesign contracts, schemas, events, and module boundaries for the complete system. |
| `apps/kiosk` | Selective reuse and redesign | Reuse sound Windows enforcement, launcher, cleanup, diagnostics, and recovery internals, but redesign its cloud/Edge contracts and UX as needed. |
| `apps/admin` | Reference only; rebuild | Use its workflows and permission coverage as discovery evidence. Do not constrain the new staff or owner UX to its routes or component structure. |
| `packages/*` | Re-evaluate per package | Generated schemas and domain vocabulary are useful inputs, but new surfaces must explicitly accept a package before depending on it. |
| Infrastructure and deployment files | Reference and redesign | Retain lessons, not compatibility; create deployment architecture for Cloud, Edge, Hub, and Local products. |

## Backend capabilities available now

| Domain | Verified capability | Reuse note |
| --- | --- | --- |
| Identity and access | Admin, staff, player, and device authentication; JWT validation; permissions; TOTP; device-bound player tokens | Reuse the security core, then introduce organization/location context, refresh-token strategy, revocation, and future SSO. |
| Devices and stations | Provisioning, registration, status, maintenance/out-of-service handling, live station data | Evolve the PC-oriented device model into Station plus attached devices and capabilities. |
| Plans and balances | Time and Happy Hours plans, purchases, balance selection, validation, top-up, deduction profiles | Reuse ledger and eligibility behavior; do not present this as full memberships or a general wallet. |
| Sessions | Start, heartbeat, reconcile, end, single-active-session enforcement, persisted balance snapshot | Reuse the session core; add pause/resume, transfer, group sessions, reservations, pricing quote linkage, and hardware lifecycle orchestration. |
| Sales and ordering | Plan/product sales; cash, recorded online, split, and credit methods; kiosk ordering and conversion | Reuse transaction behavior; a real gateway, wallet tender, refunds, tax, KOT, and recipes remain future work. |
| Staff finance | Shifts, handover, forced close, cash registers, reconciliation, deposits, expenses, credit, staff allowance | Reuse domain rules and audit patterns; rebuild operator UX. |
| Inventory and procurement | Locations, stock, receipts, adjustments, transfers, waste, vendors, reorder rules, purchase orders | Reuse services and persistence; tenant/location scoping is a prerequisite for multi-location use. |
| Catalog and media | Games, products, units, S3-compatible presigned uploads, CDN-backed kiosk presentation | Reuse proven storage behavior where useful, but define new APIs; executable trust remains local to Arena360 Station. |
| Reporting | Dashboard, revenue, usage, finance, variance, top-performer, notification, and activity data | Reuse as operational reporting; this is not yet the Intelligence product. |
| Realtime | Role-gated WebSocket, transactional outbox, deliveries, acknowledgements, rooms | Reuse the delivery foundation; define durable event contracts before Edge or external webhooks. |
| Operations | Health/readiness, metrics, OpenAPI, PostgreSQL, optional Redis, PgBouncer compatibility | Reuse; expand observability, backup/restore evidence, quotas, and regional/tenant operations. |
| Configuration | Existing configuration API plus in-flight typed setting catalog and versioned pricing policy work | Treat it as design evidence only until the new organization/location policy model is accepted and verified. |

## Kiosk capabilities available now

| Area | Verified capability | Reuse note |
| --- | --- | --- |
| Enrollment | Device fingerprinting, admin-authenticated provisioning, setup, factory reset | Retain and bind enrollment to organization/location/station assignments. |
| Player entry | Login, self-registration, plan validation, single-login conflict handling, mid-session reauthentication | Reuse suitable enforcement behavior while adding Arena360 Player and QR handoff as separate paths. |
| Session enforcement | Remaining-time UI, reconcile/heartbeat, remote end handling, process cleanup, post-restart restoration | Core Arena Agent behavior; prioritize reliability and recovery tests over visual redesign. |
| Lockdown and launch | Fullscreen shell, Windows lockdown, allow-list, game launch, process tracking, cleanup | Retain. Central catalog metadata must never silently become execution permission. |
| Offline behavior | Configurable grace, persisted session snapshot, queued end intent, reconnection reconciliation | Retain as station continuity; it is not a substitute for Arena Edge and local business operations. |
| Player experience | Game library, navigation, running-app bar, session settings, kiosk product ordering | Retain where it fits; align later with the new design system without destabilizing enforcement. |
| Operations | Diagnostics, station health, power/audio helpers, idle-only update flow | Retain and harden signed updates, health telemetry, remote commands, and fleet visibility. |

## Existing behavior to use as reference only

The current admin SPA demonstrates permission-aware workflows for players,
devices, station floor, sessions, plans, sales, products, games, kiosk orders,
shifts, cash, credit, expenses, vendors, inventory, procurement, reports,
notifications, activity, and settings. Those workflows are valuable acceptance
inputs, but the UI information architecture and implementation will be rebuilt.

## Material gaps against the vision

| Gap | Why it matters | Earliest milestone |
| --- | --- | --- |
| Complete tenant isolation and location scoping | Required before SaaS chains, owner views, quotas, or centralized policy | M01 |
| Canonical Station/capability model | Required for consoles, VR, rooms, simulators, and automation | M01 |
| New staff and owner experiences | Current admin is not a reuse target | M02–M05 |
| First-class memberships and separated wallet balances | Existing plans/balances do not meet the vision's commercial model | M04 |
| Reservations, loyalty, promotions, refunds, and payment gateway | Required for self-service and a complete commercial platform | M04–M06 |
| Arena360 Admin | No separate internal platform-administration product exists | M02 |
| Arena360 Owner | No dedicated owner application exists | M05 |
| Arena360 Staff | Existing admin app is reference-only and will not become the new staff product | M03–M04 |
| Arena360 Player Self-Service | No player-facing mobile/web application exists | M06 |
| Arena360 Marketing Website | No planned public acquisition and conversion surface exists | M11 |
| Edge replication and offline business operations | Kiosk grace covers a station, not a venue | M07 |
| Hub, console/TV control, and automation rules | No supported hardware automation product exists | M08 |
| SaaS subscription, entitlements, Local licensing, quotas, fair-use policy | Needed to sell and safely operate unlimited plans | M09 |
| Intelligence, recommendations, partner portal, public API/webhooks | Long-term optimization and ecosystem capabilities | M10 |

## Non-assumptions

- A recorded `online` payment is not a gateway integration.
- A time-plan balance is not a general cash wallet or membership.
- Kiosk offline grace is not Arena Edge.
- Existing inventory locations are not proof of organization/location tenant
  isolation.
- An OpenAPI path in an uncommitted change is not a production-ready contract.
- Existing admin screens are not approved UX for the new products.
- Existing API, schema, URL, event, or data compatibility is not required.
- Existing-data migration is handled by the separate user-owned migration tool;
  product milestones define only the clean, versioned import boundary.
