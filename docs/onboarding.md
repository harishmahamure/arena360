# Tenant venue setup

After platform provisioning creates the tenant and its first administrator, the administrator's first staff-panel landing opens `/onboarding`. The four steps are venue and location, operating settings, stations and pricing, and readiness review.

Locations are edited inside setup. Operating settings, stations, plans, and team access use their existing management forms. Save those forms before returning through the **Resume setup** card. **Save and continue**, **Back**, and **Save and exit** persist the current step in the tenant database. A browser refresh or sign-in on another device loads the same progress. Unsaved form inputs must be saved before leaving that form.

Organization administrators need `settings:read` to inspect setup and `settings:write` to save or complete it. Each management form retains its own permissions. Progress is keyed by the authenticated tenant; the API accepts no client-selected tenant identifier.

The backend requires an active venue with a valid timezone and currency, confirmation that operating settings were reviewed, a usable station, and an active plan with positive time credit whose device type, subtype, and location match that station. An active counter operator must have the required permissions at the same venue. Assigned counter permissions allow either an administrator or staff member to operate. Kiosk pairing and additional products are optional follow-up work.

Completion rechecks readiness in the same fenced SQLite transaction as the saved completion record and outbox event. It never opens a shift or creates financial transactions. The completion screen links to shift setup, where the operator chooses a location and confirms opening cash. Normal operations then proceed through player creation, plan sale, and session start.

Completion remains recorded if configuration changes later. Reopening setup displays current readiness and asks the administrator to resolve missing prerequisites before using its **Start operations** action.

The progress API is:

| Request | Purpose |
| --- | --- |
| `GET /onboarding` | Saved progress and live readiness |
| `PUT /onboarding` | Save `currentStep`, `expectedRevision`, and optional `reviewSettings` confirmation |
| `POST /onboarding/complete` | Complete setup using `expectedRevision` |

Concurrent progress writes return `409 ONBOARDING_REVISION_CONFLICT`; the panel offers to reload saved progress. Completion with missing prerequisites returns `409 ONBOARDING_NOT_READY` and readiness details. Failed writes roll back progress and its outbox event together. Existing operating tenants with shifts, transactions, or sessions are marked complete when migration 19 runs, preserving their dashboard landing.

Focused verification:

```sh
cargo test -p arena360-core --test tenant_onboarding
pnpm --filter @gaming-cafe/admin test -- src/pages/dashboard/onboarding/OnboardingPage.test.tsx src/pages/auth/LoginPage.test.tsx src/services/onboarding.test.ts
pnpm --filter @gaming-cafe/admin typecheck
```

The backend acceptance test uses authenticated HTTP requests from a fresh tenant through setup, shift/register opening, player creation, plan sale, balance purchase, and session start. Frontend tests cover first-login routing, refresh/resume, failed location/progress saves, concurrent progress, completion failure, read-only access, and tenant-separated caches.
