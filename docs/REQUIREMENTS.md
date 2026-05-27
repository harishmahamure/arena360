# REQUIREMENTS: Arena360 Gaming-Cafe Platform

> Living document. Update with every accepted user story or scope change.
> Last updated: 2026-05-27.

## Overview

**Arena360** is an end-to-end management platform for a single physical
gaming cafe (one deployment = one cafe). It combines three deployable
surfaces around one backend:

| Surface | Audience | Today | Target |
|---|---|---|---|
| `apps/backend` | All clients | NestJS 10 + Fastify, Postgres/TypeORM, JWT, R2 uploads, ZeptoMail | unchanged behaviour, hardened |
| `apps/admin` | Cafe owner, staff | Vite + React + MUI SPA (`admin.arena360.cloud`) | unchanged behaviour, shared types |
| `apps/kiosk` | Players (in-cafe) | Tauri 2 + React 19 desktop kiosk (`agent-kiosk`) | canonical kiosk; Electron variant deleted |

The platform answers four core business questions for the cafe operator:

1. **Who** is currently playing on **which device**? (sessions)
2. **What** have they purchased, and **how much** play-time is left? (plans)
3. **What** inventory and expenses move through the cafe? (POS-lite)
4. **How** healthy is the business today? (dashboard / transactions)

Players interact only with the kiosk; staff and the owner interact with
the admin SPA. The backend mediates everything and is the source of
truth for both the OpenAPI spec and the database schema.

---

## Stakeholders

### Primary

- **Cafe owner / operator** — buys the platform, configures plans,
  inventory and pricing, monitors revenue. Uses `apps/admin` from a
  desktop browser. Cares about: trustworthy numbers, low ops overhead,
  reliable kiosk lockdown.
- **Cafe staff / counter agent** — onboards walk-in players, sells
  plans, troubleshoots devices. Uses `apps/admin` from a counter
  workstation. Cares about: speed, clear errors, idempotent actions.
- **Player** — pays for a plan, sits at a gaming PC, logs in to the
  kiosk, launches a game. Cares about: instant session start, accurate
  remaining time, no UI other than the kiosk.

### Secondary

- **System integrator / DevOps** — installs the cluster, rotates
  secrets, deploys releases via Helm. Cares about: 12-factor config,
  no plaintext credentials, reproducible builds.
- **External services as actors**:
  - Cloudflare R2 (S3-compatible) for file storage / presigned uploads.
  - ZeptoMail for transactional email (OTP, receipts).
  - Future: Socket.io gateway for realtime push (see
    `docs/proposals/websocket-realtime/`).

### Out of audience (explicit non-stakeholders)

- End consumers browsing a public catalogue (no customer portal).
- Multi-tenant SaaS customers (single cafe per deployment by design).

---

## User stories

IDs follow `US-<AREA>-<NNN>`. Priority uses MoSCoW
(`Must` / `Should` / `Could` / `Won't`). Stories marked `Must` carry
acceptance criteria below.

### Auth — `US-AUTH-*`

| ID | Story | Priority |
|---|---|---|
| US-AUTH-001 | As a cafe owner, I want to log into admin with an emailed OTP, so that no shared password is reused. | Must |
| US-AUTH-002 | As the backend, I want to issue a signed JWT after OTP verification, so that subsequent API calls are stateless. | Must |
| US-AUTH-003 | As a player, I want to log into the kiosk with my registered phone/handle, so that my plan time is consumed against my account. | Must |
| US-AUTH-004 | As the backend, I want to refuse to boot when `JWT_SECRET` is missing, so that we never silently issue tokens signed with `"secret"`. | Must |
| US-AUTH-005 | As an operator, I want JWTs to expire and refresh, so that lost devices stop working within a bounded window. | Should |

### Device — `US-DEVICE-*`

| ID | Story | Priority |
|---|---|---|
| US-DEVICE-001 | As an operator, I want to register a new gaming PC by name + location, so that I can route sessions to it. | Must |
| US-DEVICE-002 | As an operator, I want to assign a curated list of installed games to a device, so that the kiosk only shows what is actually installed locally. | Must |
| US-DEVICE-003 | As an operator, I want to list all devices and their current status (free / in-session / offline), so that I can route walk-ins. | Must |
| US-DEVICE-004 | As an operator, I want to mark a device as out-of-service, so that the kiosk refuses logins on it. | Should |
| US-DEVICE-005 | As an operator, I want to bulk-import devices from CSV, so that initial onboarding is fast. | Could |

### Plan — `US-PLAN-*`

| ID | Story | Priority |
|---|---|---|
| US-PLAN-001 | As an operator, I want to create a time-based plan (e.g. 1h, 5h, 10h) with a price, so that I can sell pre-paid play time. | Must |
| US-PLAN-002 | As staff, I want to assign a plan to a player at the counter, so that they can immediately log into a kiosk. | Must |
| US-PLAN-003 | As the backend, I want to validate that a player's plan has remaining minutes before session start, so that we never start a session that can't run. | Must |
| US-PLAN-004 | As an operator, I want plans to optionally expire after N days from purchase, so that revenue does not sit on the books indefinitely. | Should |
| US-PLAN-005 | As an operator, I want a "happy hour" multiplier on plan consumption, so that I can run promotions. | Could |

### Session — `US-SESSION-*`

| ID | Story | Priority |
|---|---|---|
| US-SESSION-001 | As a player on a kiosk, I want to start a game so that a session is opened against my plan and the chosen device. | Must |
| US-SESSION-002 | As a player, I want to end my session voluntarily, so that I am only billed for time used. | Must |
| US-SESSION-003 | As the backend, I want to auto-end a session when the player's plan minutes are exhausted, so that we never overdraft a plan. | Must |
| US-SESSION-004 | As the kiosk, I want to poll session validity every 15 seconds, so that I can lock the screen within ~15 s of remote termination. | Must |
| US-SESSION-005 | As an operator, I want to force-end any active session, so that I can free a device for another player. | Should |
| US-SESSION-006 | As an operator, I want a real-time stream of session events, so that the dashboard updates without refresh. | Could (deferred to websocket-realtime proposal) |

### Inventory — `US-INVENTORY-*`

| ID | Story | Priority |
|---|---|---|
| US-INVENTORY-001 | As an operator, I want to create products (snacks, drinks, peripherals) with SKUs and prices, so that staff can sell them at the counter. | Must |
| US-INVENTORY-002 | As an operator, I want to define units (each, pack, kg), so that I can stock and sell in the right granularity. | Should |
| US-INVENTORY-003 | As an operator, I want to record purchase orders (incoming stock), so that on-hand quantities stay accurate. | Should |
| US-INVENTORY-004 | As an operator, I want to log expenses against categories, so that the dashboard reflects net revenue. | Should |
| US-INVENTORY-005 | As an operator, I want low-stock alerts on the dashboard, so that I never run out of fast-movers. | Could |

### Kiosk — `US-KIOSK-*`

| ID | Story | Priority |
|---|---|---|
| US-KIOSK-001 | As a freshly-installed kiosk, I want a one-time device-registration flow that exchanges an operator-issued code for a device JWT, so that the kiosk identifies itself to the backend on every call. | Must |
| US-KIOSK-002 | As a player at a kiosk, I want to log in with my registered identifier, so that my plan is recognised. | Must |
| US-KIOSK-003 | As the kiosk, I want to scan locally installed games (via Tauri/Rust `scan_installed_software`) and reconcile with the device's assigned game list, so that only present games are launchable. | Must |
| US-KIOSK-004 | As a player, I want to launch a chosen game and have the kiosk track its process, so that closing the game ends the session. | Must |
| US-KIOSK-005 | As the kiosk, I want to terminate a launched game on remote command, so that the operator can stop a session from admin. | Must |
| US-KIOSK-006 | As the kiosk, I want to operate in kiosk-mode lockdown (no task switcher, no shell, no system tray), so that players cannot escape to the OS. | Must |
| US-KIOSK-007 | As the kiosk, I want to display the remaining plan time prominently, so that the player is never surprised by auto-termination. | Should |
| US-KIOSK-008 | As the kiosk, I want to fall back to an offline "session-running" screen if the backend is briefly unreachable, so that a transient network blip does not boot a paying player. | Could |

### Admin — `US-ADMIN-*`

| ID | Story | Priority |
|---|---|---|
| US-ADMIN-001 | As an operator, I want a dashboard with today's revenue, active sessions, devices online, so that I can take the temperature of the business at a glance. | Must |
| US-ADMIN-002 | As an operator, I want a transactions ledger I can filter by date, player, plan, so that I can investigate disputes. | Must |
| US-ADMIN-003 | As an operator, I want to upload files (product images, plan banners) via presigned R2 URLs, so that large uploads do not transit the API server. | Must |
| US-ADMIN-004 | As staff, I want a one-screen player creation form (name, phone, optional email) with inline validation, so that walk-in onboarding takes < 30 seconds. | Should |
| US-ADMIN-005 | As an operator, I want to export transactions to CSV, so that I can reconcile with my accountant. | Should |
| US-ADMIN-006 | As an operator, I want printable receipts for plan sales, so that walk-ins get a paper trail. | Could |

---

## Acceptance criteria (Must-have stories)

### US-AUTH-001 — Owner OTP login

#### Scenario 1: Happy path
**Given** the operator has a registered admin email
**When** they submit the email at `/admin/login` and then enter the 6-digit
OTP delivered via ZeptoMail within 5 minutes
**Then** the backend returns `200` with `{ data: { accessToken, user } }`
and the admin SPA stores the token in memory + httpOnly cookie.

#### Scenario 2: Expired OTP
**Given** the operator received an OTP more than 5 minutes ago
**When** they submit it
**Then** the backend returns `401` with `error.code = "OTP_EXPIRED"` and the
SPA shows "Code expired, request a new one".

#### Scenario 3: Rate-limited request
**Given** more than 5 OTP requests have been made for the same email in 15 min
**When** another OTP is requested
**Then** the backend returns `429` with a `Retry-After` header.

### US-AUTH-004 — Fail-fast on missing JWT secret

#### Scenario 1: Missing secret
**Given** the backend is started without `JWT_SECRET` set
**When** the Nest application bootstraps
**Then** `configuration.ts` throws and the process exits with code `1`
**And** no HTTP listener is ever bound.

### US-DEVICE-001 — Register device

#### Scenario 1: Happy path
**Given** the operator is authenticated as an admin
**When** they POST `/api/v1/devices` with `{ name, location }`
**Then** the response is `201` with the new device's `id` and `registrationCode`
**And** the device appears in the device list within the same request cycle.

#### Scenario 2: Duplicate name
**Given** a device with the same name already exists
**When** the operator submits the same payload again
**Then** the backend returns `409` with `error.code = "DEVICE_NAME_DUPLICATE"`.

### US-PLAN-003 — Validate remaining minutes before session

#### Scenario 1: Sufficient time
**Given** a player has a plan with `remainingMinutes >= 1`
**When** the kiosk requests `POST /api/v1/sessions` for that player + device
**Then** a session is created and `200` is returned with the session id.

#### Scenario 2: Plan exhausted
**Given** a player has `remainingMinutes = 0`
**When** the kiosk requests session start
**Then** the backend returns `422` with `error.code = "PLAN_EXHAUSTED"`
**And** the kiosk shows "Please buy more time at the counter".

### US-SESSION-003 — Auto-end on plan exhaustion

#### Scenario 1: Background drain
**Given** an active session with `remainingMinutes` decrementing
**When** `remainingMinutes` reaches `0`
**Then** the backend marks the session `ENDED_AUTO`, persists the end time,
and within the next kiosk poll cycle (≤ 15 s) the kiosk receives
`session.status = "ENDED"` and terminates the launched game.

### US-SESSION-004 — Kiosk poll cadence

#### Scenario 1: Routine poll
**Given** an active session
**When** the kiosk polls `GET /api/v1/sessions/:id`
**Then** the response is returned within `p95 < 300 ms`
**And** the kiosk schedules the next poll exactly 15 seconds later.

### US-KIOSK-001 — First-run device registration

#### Scenario 1: Operator-issued code accepted
**Given** the operator generated a one-time `registrationCode` in admin
**When** the kiosk submits the code at first launch
**Then** the backend exchanges it for a device JWT and the kiosk persists
it via Tauri secure storage
**And** the registration code is invalidated server-side.

#### Scenario 2: Invalid code
**Given** the code has expired or been used
**When** the kiosk submits it
**Then** the backend returns `401` and the kiosk shows
"Ask the operator for a fresh registration code".

### US-KIOSK-006 — Kiosk-mode lockdown

#### Scenario 1: Task-switch blocked
**Given** the kiosk is running on a registered Windows device
**When** the user presses `Alt+Tab`, `Win`, `Ctrl+Esc`, or `Ctrl+Alt+Del`
**Then** focus stays on the kiosk window and no system UI is shown
(except the OS-level secure-attention sequence, which is unblockable).

### US-ADMIN-003 — Presigned R2 upload

#### Scenario 1: Happy path
**Given** an authenticated admin
**When** they POST `/api/v1/uploads/sign` with `{ filename, contentType, size }`
**Then** the backend returns `{ uploadUrl, publicUrl, expiresAt }` where
`uploadUrl` is a presigned R2 PUT URL valid for 5 minutes
**And** the admin SPA uploads the file directly to R2 without proxying
through the API.

#### Scenario 2: File too large
**Given** the requested `size` exceeds the configured limit of **25 MiB**
**When** the admin requests signing
**Then** the backend returns `400` with `error.code = "FILE_TOO_LARGE"`.

---

## Non-functional requirements

### Performance

- **Read endpoints** (lists, dashboard reads): `p95 < 300 ms` at expected
  load (≤ 100 RPS for a single cafe).
- **Write endpoints**: `p95 < 500 ms`.
- **Kiosk session-poll**: every 15 s ± 1 s jitter. Backend must serve
  this endpoint with `p99 < 500 ms` to keep within the polling window.
- **Dashboard initial load** (admin SPA): `LCP < 2.5 s` on a wired
  desktop connection.

### Availability

- **Target uptime**: 99.5% monthly (≈ 3.6 hours downtime / month). A
  single-cafe deployment does not warrant multi-AZ; one node + Postgres
  with daily snapshots is acceptable.
- **Planned maintenance windows**: Tuesday 04:00–05:00 local time.
- **Recovery**: RTO ≤ 4 h, RPO ≤ 24 h (last nightly snapshot).

### Security

- **All admin traffic over HTTPS** (HSTS preload-eligible). HTTP is
  redirected at the ingress.
- **JWT** signed with `HS256` using `JWT_SECRET` (>= 32 bytes). No
  default fallback (`US-AUTH-004`).
- **Helmet + compress + throttler** on Fastify boot.
- **Kiosk lockdown** blocks task switcher and shell access on Windows
  (`US-KIOSK-006`).
- **Secrets** are never committed; see `adr/0003-secrets-management.md`.

### Scalability

- Single Postgres instance (vertical scaling only) sized for ≤ 200
  active devices per deployment.
- Stateless backend pods (1–3 replicas behind ingress).
- File storage on R2 (effectively unbounded).

### Monitoring

- **Backend health**: `GET /api/v1/health` returns `{ status: "ok", db: "up" }`.
- **Metrics**: HTTP request duration histograms per route, sessions
  active gauge, plan-exhausted counter.
- **Logs**: structured JSON via `pino` (already in NestJS Fastify
  adapter), shipped to the cluster log aggregator.
- **Alerts**: page on `5xx` rate > 1% for 5 min, or `db.up = false` for
  1 min.

### Localization

- **Default timezone**: **`Asia/Kolkata`** via the `CAFE_TZ` environment
  variable. All date/time calculations (plan expiry, session timestamps,
  dashboard "today" filter) use this timezone.
- **Language**: English only in this phase.

---

## Out of scope

The following are explicitly **Won't have (now)** to keep the
consolidation tractable:

- **Customer portal** (public site, signup, online plan purchase). Was
  referenced by root scripts in the old monorepo but never built; not
  reintroducing without a fresh `US-PORTAL-*` story set.
- **Embedded uWebSockets server** inside the NestJS process. Realtime
  push is deferred to the external Socket.io gateway described in
  `docs/proposals/websocket-realtime/`.
- **Loyalty / rewards / referral programmes**.
- **Multi-tenant SaaS** (one shared backend serving many cafes). The
  schema and Helm chart assume a single cafe per deployment.
- **Mobile native apps** for players.
- **Payment-gateway integration** (online card payments). The platform
  records cash/UPI receipts only; PG integration is a future ADR.

---

## Definition of Ready

A user story is ready for development when:

- [ ] Story follows the `US-<AREA>-<NNN>` format with `As a / I want /
      So that` narrative.
- [ ] Acceptance criteria are written in Given/When/Then and cover the
      happy, alternative, and error paths.
- [ ] MoSCoW priority is assigned.
- [ ] Dependencies on other stories or external systems are listed.
- [ ] Technical approach is sketched (or marked as needing a spike).
- [ ] Story is sized (XS/S/M/L; XL must be split).
- [ ] No open clarifying questions in the story body.

## Definition of Done

A user story is done when:

- [ ] All acceptance criteria pass in an automated test (Vitest / Jest /
      cargo as appropriate).
- [ ] Code follows the project standards in
      `/Users/harishmahamure/.cursor/rules/production-grade.mdc` and the
      style enforced by Biome ^2.4.15.
- [ ] Unit tests passing; coverage on touched files does not drop below
      the current threshold (see `adr/0005-testing-strategy.md`).
- [ ] Integration tests passing where the story crosses a service boundary.
- [ ] OpenAPI spec regenerated and `packages/api-types/src/schema.ts`
      is in sync (see `adr/0004-shared-api-types-openapi.md`).
- [ ] PR reviewed and approved by at least one other engineer.
- [ ] Documentation (this file, ARCHITECTURE.md, MIGRATION.md, or the
      relevant ADR) updated if the story shifted any contract.
- [ ] Deployed to staging and acceptance criteria verified manually for
      Must-have stories.
- [ ] No new critical or high-severity security findings (npm audit,
      cargo audit, helm lint).
