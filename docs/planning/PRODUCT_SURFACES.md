# Arena360 product surfaces

Arena360 will be created as a complete, coordinated system. These surfaces
share identity, tenant context, design language, domain contracts, events, and
observability, but each has a distinct user and responsibility.

| Product | Primary users | Responsibility | Build decision |
| --- | --- | --- | --- |
| Arena360 Admin | Arena360 platform administrators, support, finance, security, operations | Tenant lifecycle, subscriptions, entitlements, platform configuration, support access, abuse controls, deployment health, and global audit | Build new |
| Arena360 Owner | Venue owners, regional managers, accountants | Organization and location governance, pricing, memberships, staff access, consolidated reporting, exceptions, and profitability | Build new |
| Arena360 Staff | Cashiers, floor staff, managers, technicians, kitchen staff | Daily venue operations: floor, sessions, players, sales, orders, shifts, cash, inventory, and maintenance | Build new |
| Arena360 Player Self-Service | Players and members | Registration, profile, wallet, membership, booking, payment, QR check-in, orders, loyalty, offers, and history | Build new |
| Arena360 Marketing Website | Prospects, partners, existing customers, search visitors | Product education, tier comparison, lead capture, demos, partner discovery, trust content, documentation entry, and conversion | Build new |
| Arena360 Station/Agent | Players and venue technicians | Device enrollment, lockdown, session enforcement, launcher, local recovery, health, and updates | Reuse selected kiosk internals; redesign contracts freely |
| Arena360 Cloud | All products | Tenant-aware domain platform, APIs, events, policy, billing, reporting, and integrations | Reuse selected backend internals; redesign contracts freely |
| Arena360 Edge | Venue managers, installers, support | Local authority, offline operations, event queue, synchronization, and device/Hub gateway | Build new |
| Arena360 Hub | Installers and technicians | Secure physical control of TVs, inputs, relays, and supported venue hardware | Build new |
| Arena360 Intelligence | Owners and regional managers | Trusted profitability, demand, retention, and pricing recommendations | Build new |

## Identity and access boundaries

- Arena360 Admin is an internal platform product. It must not be implemented as
  an elevated Owner screen.
- Arena360 Owner governs one or more authorized organizations and locations.
- Arena360 Staff performs location operations with narrow, action-level
  permissions and shift context.
- Arena360 Player Self-Service uses customer identity and cannot inherit staff
  or operator privileges.
- Support impersonation or assisted access must be time-bound, reasoned,
  visible, and audited.
- The Marketing Website is public and isolated from authenticated operational
  applications, except for explicit sign-in and lead/demo handoffs.

## Shared platform capabilities

The products should share definitions rather than implementations by default:

- Organization → Location → Zone → Station hierarchy.
- Identity, session, permission, entitlement, and audit vocabulary.
- Pricing quotes and explanations.
- Money, wallet, membership, loyalty, and payment state models.
- Reservation, session, order, and automation state machines.
- Event envelope, correlation, idempotency, and error conventions.
- Design tokens, content standards, accessibility targets, and analytics names.

Shared packages may be created for the new system, but the current frontend
packages are not compatibility constraints.

The separate existing-data migration tool is a delivery utility, not an
Arena360 product surface. It targets versioned new-system import contracts and
does not influence product navigation or preserve legacy models.

## Navigation between products

Each authenticated product has its own URL space and deployment artifact.
Cross-product links must preserve safe tenant/location context, use a common
identity handoff, and never rely on sharing browser storage. Public marketing
pages link to sign-in, demo, sales, support, and documentation entry points.
