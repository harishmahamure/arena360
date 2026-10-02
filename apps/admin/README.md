# Arena360 admin workspace

A responsive React admin and staff workspace for venue operations. The redesigned shell and admin theme cover every existing route, with permission-aware navigation, page search (Cmd/Ctrl K), mobile navigation, and shared list, form, and detail layouts. The kiosk has its own theme.

## Run

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm admin:dev
```

Vite serves the app at `http://localhost:5173`. Configure `VITE_API_URL` and `VITE_GATEWAY_URL` using `.env.example` for your backend. The app uses the existing protobuf transport, authentication, permissions, and shift rules; it does not seed sample data or bypass authentication.

## Configuration

- General settings are searchable by key or description and grouped by category.
- Organization defaults and location overrides show their inheritance source.
- Edits stay local until reviewed and saved with an audit reason. The original revision is retained while editing to detect concurrent changes.
- Changes save individually because the backend has no batch endpoint. On partial failure, successfully saved fields clear and remaining edits stay available.
- Scope changes prompt before discarding edits; reload and normal link navigation warn about unsaved changes.
- Pricing policies have visual and advanced JSON editors. Drafts must be saved before validation, simulation, and publication.
- History supports restoring values with a new audit reason.

## Verification

```sh
pnpm --filter @gaming-cafe/admin typecheck
pnpm --filter @gaming-cafe/admin test
pnpm --filter @gaming-cafe/admin build
pnpm --filter @gaming-cafe/ui typecheck
```

The tests cover authentication/MFA, realtime handling, route permissions, notifications, statistics, numeric validation, concurrent configuration revisions, partial saves, and scope changes. Browser checks include mocked failure states and live protobuf API integration against an isolated database.

Workspace package source is resolved consistently in Vite, Vitest, and TypeScript to avoid depending on stale generated package builds. Page modules are loaded on demand.

## Appearance and sessions

The palette button in the top bar controls light/dark/system mode, custom accent colors, comfortable/compact density, and rounded/square corners. Accent colors are adjusted for readable text contrast. Preferences are validated and stored per account in this browser; they are not organization settings or synchronized between devices.

Panel access is bootstrapped from an unexpired admin/staff JWT. Operational capabilities follow the backend's existing role model; settings and pricing require the corresponding organization grants. Pricing read, edit, and publish permissions are independent. The backend remains authoritative on every protected request and verifies current account status, role, and active organization membership. `/auth/me` revalidates the profile on focus and once per minute.

JWT expiry, protected API 401s, WebSocket session rejection, and inactivity sign the user out. `VITE_SESSION_IDLE_MINUTES` sets inactivity timeout (default 30 minutes, valid range 1–1440); activity across tabs extends it. A stale request from a previous token cannot log out a newer session. Logout clears the token, persisted identity, query cache, and realtime connection across tabs. Financial shifts are not automatically closed by expiry; staff can sign in and resume them.

## API and realtime integration

The API defaults to `http://localhost:3000`. The WebSocket URL is derived from that address unless `VITE_GATEWAY_URL` supplies an explicit override. Deploy the frontend and backend changes together, including `/auth/me` and the `configuration` realtime channel. Backend schema prerequisites remain the existing configuration-policy migrations; this change adds no migration.

Successful API mutations invalidate cached views, including forms that call services directly. Other tabs receive a refresh signal. Live events refresh active queries and mark inactive queries stale, so details, aggregates, and lists stay aligned. When realtime is unavailable, visible views refresh every 30 seconds while reconnection continues.

The protobuf WebSocket client uses bounded reconnect backoff, a connection timeout, 25-second heartbeats, and a 60-second reply timeout. Each login creates a new connection, including in React StrictMode. The server closes connections at JWT expiry and checks panel account access every 30 seconds. Settings/pricing events require the relevant read grant and matching organization for both live delivery and replay. Durable delivery is recorded before sending, and the client acknowledges only after event handlers run successfully.

Additional verification:

```sh
pnpm --filter @gaming-cafe/utils test
pnpm --filter @gaming-cafe/proto typecheck
cargo test --manifest-path apps/backend/Cargo.toml --lib realtime
# Use an isolated, migrated test database for database acceptance tests.
DATABASE_URL=... cargo test --manifest-path apps/backend/Cargo.toml --test panel_session --test configuration_policy -- --ignored
```

For live API testing, set both `DATABASE_URL` and an explicit isolated `REDIS_URL`; an empty Redis URL falls back to the shared localhost Redis server.

Live verification on an isolated migrated database covered login, invalid credentials, theme persistence, 29 main routes, cross-tab logout, settings read/write and revision conflicts, pricing draft/validate/simulate/publish, staff authorization, account deactivation, and WebSocket expiry. External object-storage uploads still require configured storage credentials.

Additional live workflows verified player creation with immediate list refresh, remote configuration updates through WebSocket, mobile theme editing, staff shift/register start, zero-cash shift closure, denied staff navigation, and proactive browser logout at JWT expiry. Shared forms now expose accessible labels for text, password, numeric, and choice fields.

## Guided forms

Admin entry and editing flows use validated steps followed by review and an explicit final action. This covers players, devices, plans, products, games, sessions, transaction status, plan/POS checkout, vendors, expenses, credit limits and settlement, passwords, inventory locations/counts/transfers/waste/reorder rules/receipts, purchase orders, register adjustments, and shift opening/closure/handover. Rejection dialogs also review the reason before submitting. Search/filter controls, instant appearance preferences, and authentication challenges retain their direct interactions.

- Shared `FormBuilder` consumers opt in with `wizard`. Existing `sections` become steps; `wizardSteps` assigns named fields to explicit groups. Unassigned fields remain available in an additional step. Read-only views remain read-only. Passwords and verification codes are masked in review.
- Stateful workflows use `components/GuidedForm` and `GuidedStep`. A step's synchronous `validate` function handles business constraints alongside native required/type validation. Put final mutation buttons in `actions`; they are available only at review. Use `data-wizard-cancel` on cancellation buttons, and `busy` to lock controls during requests. Dialog dismissal must also be disabled while busy. A custom `review` can show a concise summary instead of all fields.
- Step contents stay mounted to retain search selections, uploaded assets, and other local drafts. Final validation returns to the first invalid step. Shared select labels have unique IDs even when multiple steps are mounted.
- Configuration is grouped by category, with at most four settings per step. Currency and timezone controls offer searchable choices. Search/category shortcuts preserve drafts, revision checks, scope-discard confirmation, audit reasons, and partial-save recovery. Unsaved highlighting follows the active theme; the mobile save area stays in document flow.
- Pricing uses choose policy → rates/rules → validate/simulate → publication → review. Draft creation and draft saves are intentional intermediate API operations; publishing remains a separate final action.

Wizard regression tests cover required fields, back navigation, cross-step validation, password masking, search selection/reset, deferred submission, and zero-cash closure. Live browser verification against the isolated QA backend covered player/vendor creation, pricing navigation, shift opening/closure, and 14 entry/inventory routes at a 390px viewport.

### Kitchen operations and financial reporting

- `/kitchen` is the shared staff/admin preparation board. An administrator opts products into kitchen preparation in **Menu setup**, assigns a station, and sets a preparation target through a review wizard. New completed/credit sales enqueue those products atomically with the sale; a pending sale enqueues on completion. A unique transaction reference prevents duplicate tickets. Product/station snapshots preserve the original ticket. Customer kiosk notes carry through to the sale and kitchen.
- Tickets move queued → preparing → ready → served. Cancellation requires a reason and never refunds money or restores stock. Every transition stores an actor/time event and checks the expected revision; terminal tickets cannot reopen. Actions apply to the whole ticket, even when filtered by station. Recent history covers seven days/500 tickets; the live queue prioritizes the oldest 500. Events refresh connected panels, with 15-second polling as a fallback.
- `/finance/reports` is admin-only. Inclusive UTC date ranges (maximum 366 days) aggregate the complete ledger in one database snapshot. Sales reflect current payment status and original transaction date, so historical totals can restate after a refund. Credit settlement receipts are separate from booked sales. Receivables are current, not historical. Approved expenses use expense date; pending expenses remain separate. CSV exports preserve decimal strings, quote fields, and escape spreadsheet formulas.
- These are operational reports, not general-ledger accounting, recognized revenue, tax returns, or a P&L. The existing sales ledger has no tenant column: these new APIs fail closed outside the original venue. Multi-venue ledger segregation, recipe/ingredient consumption, station-level item fulfillment, and accounting integrations are not implemented by this module.

Backend rollout requires migration `20261002130000_kitchen_operations.sql` before starting the updated API. Apply it using the repository migration workflow (`pnpm migration run`) against the intended environment. No historical tickets are backfilled. API routes use the existing authenticated gRPC-web gateway:

- `GET /kiosk-orders/kitchen/tickets?history=false`
- `PATCH /kiosk-orders/kitchen/tickets/:id` with `status`, `expectedRevision`, and cancellation `reason`
- Admin: `GET /kiosk-orders/kitchen/menu`, `PUT /kiosk-orders/kitchen/menu/:productId` with `enabled`, `station`, `prepMinutes`, `expectedRevision`
- Admin: `GET /stats/finance/report?startDate=YYYY-MM-DD&endDate=YYYY-MM-DD`

Run `cargo test --test kitchen_finance -- --ignored` with an isolated migrated `DATABASE_URL` for atomic enqueue/report integration coverage. Fixtures roll back. Admin tests include kitchen review/cancellation/payment guards and CSV export safety.
