# UX planning scaffold

The UX goal is not one universal dashboard. Arena360 needs purpose-built
experiences that share concepts, permissions, and visual language while fitting
very different operating contexts.

## Product surfaces

| Surface | Primary users | Context | Planned status |
| --- | --- | --- | --- |
| Arena360 Admin | Arena360 platform operations, support, security, finance | Cross-tenant platform administration with strict privileged-access controls | Build new |
| Arena360 Owner | Owner, regional manager, accountant | Multi-location control, configuration, exceptions, trends | Build new |
| Arena360 Staff | Cashier, floor staff, manager, technician, kitchen staff | Fast, interruption-heavy venue operations | Build new |
| Arena360 Player Self-Service | Player/member | Mobile-first self-service before and during a visit | Build new |
| Arena360 Marketing Website | Prospects, partners, customers | Public product education, trust, discovery, and conversion | Build new |
| Station/Kiosk | Player and on-site technician | Fullscreen gaming PC, controller/keyboard friendly | Reuse and evolve |
| Edge Console | Installer, manager, support | Local appliance setup, sync, recovery, diagnostics | Build new |
| Hub Setup | Installer, technician | Pair devices, learn IR, map inputs, test actions | Build new |
| Partner Portal | Certified partner and support | Customer onboarding, installation, license and support workflows | Build later |

## Shared UX principles

1. Show the venue state before navigation: station availability, active
   sessions, offline devices, pending orders, and cash exceptions.
2. Prefer explicit state transitions over generic edit forms.
3. Explain price, balance selection, permission denial, and automation outcomes.
4. Make risky actions state their effect and require a reason where audit is
   required.
5. Preserve work during reconnects and distinguish stale, queued, synced, and
   failed data.
6. Support keyboard-first staff operation, touch targets, color-independent
   status, readable timers, and WCAG 2.2 AA for web surfaces.
7. Never rely on toast-only confirmation for money, session, or hardware state.
8. Use the vocabulary Organization → Location → Zone → Station consistently.

## Arena360 Admin information architecture

```text
Platform
├── Organizations and locations
├── Subscriptions and entitlements
├── Deployments and service health
├── Usage, quotas, and abuse controls
└── Feature and policy rollout

Operations
├── Support cases and assisted access
├── Billing exceptions
├── Security events
├── Global audit
└── Partner administration
```

Admin privileges must be explicit, time-bound where practical, and separated
from customer-owned Owner and Staff roles.

## Arena360 Staff information architecture

```text
Today
├── Station floor
├── Active sessions
├── Queue and reservations
├── Orders
└── Alerts

Sell
├── Quick sale
├── Plans and memberships
├── Wallet recharge
└── Transaction history

Operate
├── Players
├── Stations and devices
├── Inventory and kitchen
├── Shifts and cash
└── Maintenance

Manage
├── Catalog
├── Pricing and promotions
├── Staff and permissions
├── Reports
└── Venue settings
```

The default route should be role-aware. A cashier lands on Today or Sell; a
floor operator lands on the station floor; a kitchen user lands on orders.

## Arena360 Owner information architecture

```text
Portfolio
├── Location health
├── Revenue and utilization
├── Exceptions
└── Recommended actions

Business
├── Locations and zones
├── Pricing templates
├── Memberships and loyalty
├── Staff access
└── Consolidated reports

Platform
├── Subscription and entitlements
├── Integrations
├── Audit and exports
└── Deployment health
```

## Arena360 Player Self-Service information architecture

```text
Play
├── Locations and availability
├── Reservations
├── QR check-in
└── Active session

Account
├── Wallet and payments
├── Memberships and loyalty
├── Orders
├── Offers
└── Visit and transaction history
```

## Arena360 Marketing Website information architecture

```text
Product
├── Business
├── Automation
├── Intelligence
├── Cloud and Local
└── Integrations

Solutions
├── Small cafés
├── Gaming arenas
├── Chains and franchises
└── Installation partners

Trust and conversion
├── Pricing
├── Security and reliability
├── Customer stories
├── Documentation and support
├── Book a demo
└── Sign in
```

## Critical journeys to prototype before implementation

| Journey | Success signal | Failure states that must be designed |
| --- | --- | --- |
| Start a walk-in session | Staff can find a station, see the explained price, take payment, and start it without leaving the flow | Station changes state, payment fails, stale quote, insufficient balance, permission denied |
| End or extend a session | Customer and staff see one authoritative result and station cleanup status | Kiosk offline, cleanup delayed, balance conflict, duplicate request |
| Sell a membership or wallet recharge | Tender and resulting entitlement are unambiguous | Partial payment, gateway pending, duplicate callback, refund |
| Reserve and check in | Availability, deposit, grace, and station assignment are visible | Conflict, late arrival, no-show, location offline |
| Close a shift | Expected cash, actual cash, variance, and required approvals are clear | Unconverted orders, open sessions, pending deposit, offline records |
| Handle internet loss | Staff know what remains available, what is queued, and when sync completes | Split-brain conflict, expired local authority, unrecoverable action |
| Configure pricing | Owner can simulate, understand, approve, schedule, and roll back a rule set | Overlap, invalid scope, unexpected quote delta, stale version |
| Pair Hub hardware | Installer can identify a physical device, test actions, and prove safe fallback | Duplicate device, wrong TV, command timeout, offline Edge |
| Admin resolves a tenant issue | Support can diagnose and assist without permanent or invisible access | Wrong tenant, expired access, sensitive data exposure, unaudited mutation |
| Prospect chooses a product | Visitor understands fit, deployment, price, and next action | Unsupported promise, unclear fair use, broken lead handoff, inaccessible content |

## Floor and station model

The floor view is the operational heart of the product. Every station card
needs:

- stable name and type;
- current state and freshness;
- active customer or reservation when permitted;
- elapsed/remaining time;
- price or plan context;
- device, Edge, and hardware health;
- the next valid actions for the current role.

Station states must use a shared state model: `AVAILABLE`, `RESERVED`,
`STARTING`, `ACTIVE`, `PAUSED`, `ENDING`, `CLEANUP`, `MAINTENANCE`, and
`OFFLINE`. Product discovery must define who can cause each transition,
timeouts, and recovery paths before UI build.

## UX deliverables per milestone

Each milestone that changes a user journey must provide:

- journey map and happy-path prototype;
- empty, loading, stale, permission-denied, offline, and failure states;
- role and device-size matrix;
- content and terminology review;
- accessibility review;
- usability evidence with representative users;
- analytics events needed to measure adoption and failure.

High-fidelity visual design is not an entry requirement. State completeness and
workflow validation are.
