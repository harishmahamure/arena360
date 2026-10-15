What this repo does: Arena360 runs a gaming cafe: staff sell plans and products, players use Windows kiosks, and Android TV stations follow console sessions. A Rust API stores users, time wallets, stock, payments, shifts and notifications in Postgres, with Redis caching and WebSocket updates. This audit assumes one busy venue, tens of stations, simultaneous counter operations, intermittent station networks, and 2–4 backend replicas, as configured in the Helm chart.

# Repository audit — 2026-10-15

Scope: the entire working tree at commit `8ebd74630a554c7e0ff417f58c6f6bb38ed77d89`, not recent changes. This is a risk-based audit of a large repository, not a claim that every line was read. Only this report was added; source code, dependencies, migrations and deployed systems were not changed. Pre-existing changes to `AGENTS.md`, `README.md` and `.agents/` were left alone.

There are **20 findings: 16 P1 and 4 P2**. No P0 or P3 finding is asserted. P0 means an immediate, unconditional critical incident; P1 means serious access, money or operational harm; P2 means a material reliability or maintenance defect; P3 means a small improvement. Severity reflects this application's exposure, not a package scanner's label. Source-derived failure cases below were not exercised against a live database or production service.

## Must fix

### 1. P1 — A tracked user export contains authentication data

**Location:** [`users_202606150529.csv:1`](users_202606150529.csv#L1), data begins at line 2.

- **What this is:** A Git-tracked CSV contains 762 user records, with 762 bcrypt password hashes, 762 nonempty `sessionOtp` fields and 453 nonempty email fields. These counts were checked with a CSV parser; no credential or personal value is reproduced here.
- **Problem / evidence:** Anyone with repository or checkout access can copy these fields and attempt password cracking offline. The file is an identifiable account export, rather than an anonymized fixture; whether the credentials still work was not tested.
- **Recommended fix:** Remove the export from distribution and Git history through a coordinated cleanup, replace any needed fixtures with synthetic data, and reset affected credentials if these are real accounts. Deleting the current file alone does not remove historical copies.
- **If we skip it:** Repository access continues to disclose account data and password verifiers.

### 2. P1 — Staff can promote themselves to administrator

**Location:** [`apps/backend/src/handlers/users.rs:74`](apps/backend/src/handlers/users.rs#L74), [`services/user_service.rs:125`](apps/backend/src/services/user_service.rs#L125), [`repositories/user_repo.rs:158`](apps/backend/src/repositories/user_repo.rs#L158).

- **What this is:** `PUT /users/{id}` accepts an `AdminOrStaff` token and a body containing `role` and `isActive`.
- **Problem / evidence:** A staff member can submit `{"role":"admin"}` for their own ID: the service never checks the caller's authority over roles, and SQL writes the supplied value directly. Their next admin login uses their existing password and the new role; staff without TOTP do not gain a TOTP requirement automatically.
- **Recommended fix:** Pass caller claims into the update policy and restrict role changes and editing privileged accounts to admins. Add an HTTP regression test proving staff cannot promote themselves, promote another account, or edit an admin.
- **If we skip it:** A counter login grants full configuration, account and financial administration.

### 3. P1 — Public TV pairing can expose a staff or admin login token

**Location:** [`apps/backend/src/handlers/auth.rs:254`](apps/backend/src/handlers/auth.rs#L254), [`realtime/acl.rs:51`](apps/backend/src/realtime/acl.rs#L51), [`handlers/auth.rs:204`](apps/backend/src/handlers/auth.rs#L204), [`services/auth_service.rs:279`](apps/backend/src/services/auth_service.rs#L279).

- **What this is:** The public pairing endpoint issues a device-channel token after receiving only a PlayStation device UUID. The SSO endpoint publishes a redeemable login secret on that channel, and redemption issues the creator's normal staff/admin JWT.
- **Problem / evidence:** Someone who knows a TV UUID can obtain a pairing token, subscribe to its channel, and capture the next SSO secret sent for that TV; redemption does not enforce the stored purpose or device binding. This requires an SSO issuance, so it is conditional, but the endpoints remain routed and usable even though the current TV client stopped using them.
- **Recommended fix:** Require a short-lived pairing challenge approved on the physical device, and exchange an SSO grant only for the narrowly intended device operation. If legacy clients can be retired, remove these endpoints and their DTOs/OpenAPI entries; [DRAFT-0037](docs/adr/DRAFT-0037-console-tv-kiosk-style-provisioning.md) explicitly retains them for compatibility, so do not assume removal is safe without checking the fleet.
- **If we skip it:** A legacy provisioning/login action can disclose the issuing operator's full API privileges.

### 4. P1 — Low-privilege tokens can read other users and financial records

**Location:** [`apps/backend/src/handlers/users.rs:51`](apps/backend/src/handlers/users.rs#L51), [`handlers/transactions.rs:32`](apps/backend/src/handlers/transactions.rs#L32), [`handlers/transactions.rs:55`](apps/backend/src/handlers/transactions.rs#L55), [`handlers/devices.rs:34`](apps/backend/src/handlers/devices.rs#L34).

- **What this is:** Several read handlers rely only on the router's general JWT check. They have no role or record-ownership extractor.
- **Problem / evidence:** A device, player, or pairing JWT can call `GET /transactions` without a player restriction, read transaction details, enumerate devices, and fetch a user by UUID. The user serializer hides password/TOTP secrets but still exposes names, phone and credit information; the transaction repository applies only caller-supplied filters.
- **Recommended fix:** Require `AdminOrStaff` for staff-only lists and financial details, and enforce self-only access where player reads are intended. Add a negative access matrix covering pairing, device, player and staff tokens; keep explicitly needed kiosk catalogue reads available.
- **If we skip it:** One station or player credential can disclose venue-wide financial and personal information.

### 5. P1 — Disabling an account does not revoke its existing access

**Location:** [`apps/backend/src/middleware/auth.rs:45`](apps/backend/src/middleware/auth.rs#L45), [`middleware/auth.rs:137`](apps/backend/src/middleware/auth.rs#L137), [`realtime/handler.rs:72`](apps/backend/src/realtime/handler.rs#L72), [`infra/helm/backend/values-prod.yaml:31`](infra/helm/backend/values-prod.yaml#L31).

- **What this is:** HTTP and WebSocket authentication trust signed role claims until token expiry. Production access tokens last 24 hours, and WebSocket connections validate expiry only during upgrade.
- **Problem / evidence:** After an admin disables or demotes a staff account, its already-issued JWT still passes role extractors and can continue accessing records or changing settings. An established socket can remain subscribed beyond JWT expiry; neither the connection loop nor dispatcher rechecks account state or expiry.
- **Recommended fix:** Check active account state and a revocation/version marker for privileged requests, invalidate that state on account changes, and close sockets on expiry or revocation. Cover disable, demotion and password reset with existing-token tests.
- **If we skip it:** Removing a compromised user's access in the admin UI does not reliably stop that user's current sessions.

### 6. P1 — Public password and TOTP login attempts are not throttled

**Location:** [`apps/backend/src/handlers/auth.rs:31`](apps/backend/src/handlers/auth.rs#L31), [`handlers/auth.rs:61`](apps/backend/src/handlers/auth.rs#L61), [`services/auth_service.rs:548`](apps/backend/src/services/auth_service.rs#L548).

- **What this is:** Admin and staff login endpoints are public and run synchronous bcrypt verification inside async request processing.
- **Problem / evidence:** Repeated requests for a known username can keep trying passwords or TOTP codes and consume API worker CPU. Searches of router, middleware, login services and ingress values found no login rate limiter; the `qps` JWT field is descriptive only, and the separate OTP limiter is not called by these login handlers.
- **Recommended fix:** Add an atomic, shared attempt budget by account and client address before bcrypt/TOTP work, with bounded behavior during cache outages. Run bcrypt work through `spawn_blocking` with bounded concurrency and test repeated and concurrent failures.
- **If we skip it:** Login credentials can be attacked without an application-enforced attempt limit, and a login burst can delay unrelated cafe operations.

### 7. P1 — Concurrent session requests can deduct the same minutes twice

**Location:** [`apps/backend/src/services/session_service.rs:208`](apps/backend/src/services/session_service.rs#L208), [`services/session_service.rs:584`](apps/backend/src/services/session_service.rs#L584), [`services/session_service.rs:649`](apps/backend/src/services/session_service.rs#L649), [`repositories/session_repo.rs:392`](apps/backend/src/repositories/session_repo.rs#L392).

- **What this is:** Heartbeats and session endings compute the uncharged time from a session record, debit the wallet, and then update the session's charged total through separate queries.
- **Problem / evidence:** Two requests that both read 10 charged minutes and calculate 11 each debit one minute before both store 11. The end query's `endTime IS NULL` guard runs after deduction, so a losing concurrent end can still charge the wallet; a failure between debit and checkpoint has the same retry hazard.
- **Recommended fix:** Lock the session row, read its latest charged total, debit the wallet, append its ledger entry and update/end the session in one database transaction. Add concurrent heartbeat/heartbeat and heartbeat/end tests, plus a rollback test between debit and checkpoint.
- **If we skip it:** Players lose paid time through duplicate requests, overlapping clients or transient database failures.

### 8. P1 — Purchases can partially commit, and payment completion can grant benefits twice

**Location:** [`apps/backend/src/services/transaction_service.rs:453`](apps/backend/src/services/transaction_service.rs#L453), [`services/transaction_service.rs:285`](apps/backend/src/services/transaction_service.rs#L285), [`services/transaction_service.rs:543`](apps/backend/src/services/transaction_service.rs#L543), [`services/balance_service.rs:180`](apps/backend/src/services/balance_service.rs#L180).

- **What this is:** Sale records, plan recharges, wallet ledger entries and cash-register entries are committed through separate service calls. Product sales commit stock and the sale before adding the till entry.
- **Problem / evidence:** A closed register or failed recharge can return an error after the sale is already stored; retrying can record another sale. Two concurrent `pending → completed` updates can both read the old pending state and each grant minutes/cash effects, because the status update and side effects are not one locked operation and no per-transaction benefit uniqueness constraint exists.
- **Recommended fix:** Share one database transaction across the sale, benefit/stock, ledger and till writes; lock payment transitions and enforce unique effect references. Publish events and refresh caches after commit, without turning a successful database commit into an ambiguous failed purchase response.
- **If we skip it:** Revenue, stock, time balances and cash totals can disagree, while retries can duplicate customer benefits or recorded sales.

### 9. P1 — Concurrent credit purchases can exceed the player's credit limit

**Location:** [`apps/backend/src/services/credit_service.rs:46`](apps/backend/src/services/credit_service.rs#L46), [`services/credit_service.rs:54`](apps/backend/src/services/credit_service.rs#L54), [`services/transaction_service.rs:435`](apps/backend/src/services/transaction_service.rs#L435).

- **What this is:** Credit eligibility reads a cached outstanding amount, compares it with the limit, and inserts the purchase later.
- **Problem / evidence:** With ₹100 available, two simultaneous ₹80 purchases can both pass the same cached check and commit ₹160 of new debt. The product path's product locks do not serialize purchases for the same player across different products, and cache invalidation happens after the decision.
- **Recommended fix:** Lock the player's credit/account row and calculate outstanding credit from the database inside the purchase transaction. Make every credit purchase and limit-changing path use that lock, and test two concurrent purchases against one limit.
- **If we skip it:** The displayed credit cap does not reliably limit what the venue lends.

### 10. P1 — The API accepts completed payments whose cash and online amounts do not add up

**Location:** [`apps/backend/src/services/transaction_service.rs:97`](apps/backend/src/services/transaction_service.rs#L97), [`services/transaction_service.rs:446`](apps/backend/src/services/transaction_service.rs#L446), [`services/transaction_service.rs:594`](apps/backend/src/services/transaction_service.rs#L594).

- **What this is:** Payment normalization supplies missing cash/online values but preserves supplied ones. It validates payment labels and online reference digits, not the payment sum.
- **Problem / evidence:** A completed ₹100 sale with `paymentMethod: "split_payment", cashAmount: 1, onlineAmount: 0` is accepted, grants the purchase and records only ₹1 of cash. The admin form checks this sum in `containers/sales/paymentSplit.ts`, but direct API requests bypass that check; the transaction schema has no matching sum constraint.
- **Recommended fix:** Validate nonnegative payment components and their sum against the resolved server total before any write; derive full amounts for single-method payments. Reuse the settlement flow's validation rules where appropriate and add direct API tests with missing, negative and mismatched components.
- **If we skip it:** Completed sales and drawer totals can disagree even without a database failure.

### 11. P1 — Two session starts can claim the same wallet or device

**Location:** [`apps/backend/src/services/session_service.rs:283`](apps/backend/src/services/session_service.rs#L283), [`services/session_service.rs:410`](apps/backend/src/services/session_service.rs#L410), [`repositories/session_repo.rs:334`](apps/backend/src/repositories/session_repo.rs#L334).

- **What this is:** Starting gameplay checks for an existing session and an available device, then inserts a session and updates the device separately.
- **Problem / evidence:** Two simultaneous starts can both see no session and an available device, then both insert an open session. Migrations add ordinary lookup indexes but no unique open-session constraint; the cross-device player check also happens before insertion and outside a lock.
- **Recommended fix:** Lock the player, balance and device in a consistent order during start, recheck availability, and insert/update together. Add partial unique indexes for open sessions per device and balance, plus a player-level serialization rule that also covers players with multiple wallets.
- **If we skip it:** A player can have simultaneous sessions and one station can be assigned twice, confusing billing and remote controls.

### 12. P1 — Offline logout is billed until the later replay time

**Location:** [`apps/kiosk/src/context/KioskProvider.tsx:359`](apps/kiosk/src/context/KioskProvider.tsx#L359), [`context/KioskProvider.tsx:1041`](apps/kiosk/src/context/KioskProvider.tsx#L1041), [`apps/backend/src/handlers/kiosk.rs:189`](apps/backend/src/handlers/kiosk.rs#L189), [`services/session_service.rs:644`](apps/backend/src/services/session_service.rs#L644).

- **What this is:** Failed kiosk logout is queued locally with `queuedAt`, the player is signed out, and the end is replayed with a reason only.
- **Problem / evidence:** If a player stops at 14:00 while offline and the intent is replayed at 15:00, the API supplies `end_time: None` and bills through the server's 15:00 clock. The queued stop time never reaches the server, and replay is restricted to the same player's authentication, so reconnection by itself may not close the abandoned session.
- **Recommended fix:** Persist the stop instant and enough device-bound evidence to reconcile it safely, and define server-side bounds against session start and last accepted heartbeat. Allow authenticated device recovery of its queued ends without waiting for that player to return; test a long network outage followed by replay.
- **If we skip it:** A player who already stopped playing can lose an extra hour or the rest of their wallet.

### 13. P1 — Converting a kiosk order records a sale but leaves the order open

**Location:** [`apps/backend/src/services/kiosk_order_service.rs:379`](apps/backend/src/services/kiosk_order_service.rs#L379), [`handlers/kiosk_orders.rs:181`](apps/backend/src/handlers/kiosk_orders.rs#L181), [`handlers/transactions.rs:102`](apps/backend/src/handlers/transactions.rs#L102).

- **What this is:** `/kiosk-orders/{id}/convert` builds and creates a product transaction. The separate generic transaction handler contains the fulfillment call.
- **Problem / evidence:** The conversion service returns immediately after `transactions.create`; neither it nor its handler calls `mark_fulfilled`. A successful conversion therefore leaves the order pending/preparing with no linked transaction, so repeating conversion creates another sale and deducts stock again.
- **Recommended fix:** Move order validation and fulfillment into the same database transaction as order-backed sale creation, and use it from both entry points. Lock the order and return its existing sale on retries; test that conversion closes the order and repeated/concurrent conversion creates exactly one sale.
- **If we skip it:** Staff see an already-sold order as open, duplicate charges/stock deductions are possible, and the player remains blocked by the one-open-order rule.

### 14. P1 — Cash expense approval silently skips the cash-register debit

**Location:** [`apps/backend/src/services/expense_service.rs:189`](apps/backend/src/services/expense_service.rs#L189), [`handlers/expenses.rs:158`](apps/backend/src/handlers/expenses.rs#L158), [`handlers/auth.rs:42`](apps/backend/src/handlers/auth.rs#L42).

- **What this is:** Expense approval is admin-only, but its cash debit searches for the approving admin's active shift. Admin login explicitly closes orphan admin shifts, and counter operations are staff-only.
- **Problem / evidence:** Under normal policy an approving admin has no active shift, so a cash expense becomes approved while all debit code is skipped. Lookup/write errors are also swallowed; even if a register exists, a split expense debits its entire amount rather than a distinct cash portion.
- **Recommended fix:** Resolve the expense's actual cash source/register explicitly instead of the approver's shift, and commit approval plus the correct cash debit together. Reject an unspecified cash source before approval and add a test using an admin without a shift.
- **If we skip it:** Approved expenses reduce reported profit without reducing expected drawer cash, causing unexplained variances.

### 15. P1 — Durable WebSocket events disappear when the recipient is offline

**Location:** [`apps/backend/src/realtime/dispatcher.rs:92`](apps/backend/src/realtime/dispatcher.rs#L92), [`realtime/deliveries.rs:66`](apps/backend/src/realtime/deliveries.rs#L66), [`realtime/connection.rs:64`](apps/backend/src/realtime/connection.rs#L64).

- **What this is:** A durable event is intended to survive a disconnect. Delivery rows are currently created only while iterating matching live connections, and reconnect replay reads only those delivery rows.
- **Problem / evidence:** If a station is disconnected when its durable `session.ended` event is published, no delivery row is inserted for it and reconnect replay cannot find that event. Dispatcher database/listener outages also have no persisted catch-up cursor; the TV client does not fetch current session state on WebSocket reconnect.
- **Recommended fix:** Persist intended recipient delivery independently of live sockets, or implement authorized cursor-based replay from the outbox. Also reconcile current station session state on reconnect and test publishing an end while the device is disconnected.
- **If we skip it:** A reconnecting station can keep showing or controlling a session the server already ended, despite the event being marked durable.

### 16. P1 — TV auto-end silently fails and stops trying

**Location:** [`apps/console-tv/app/src/main/java/com/gamingcafe/consoletv/data/ApiClient.kt:270`](apps/console-tv/app/src/main/java/com/gamingcafe/consoletv/data/ApiClient.kt#L270), [`ConsoleTvViewModel.kt:452`](apps/console-tv/app/src/main/java/com/gamingcafe/consoletv/ConsoleTvViewModel.kt#L452), [`ConsoleTvViewModel.kt:491`](apps/console-tv/app/src/main/java/com/gamingcafe/consoletv/ConsoleTvViewModel.kt#L491).

- **What this is:** The TV timer sends one end request when remaining time reaches the cutoff, then breaks its timer loop. Local session teardown and HDMI switching wait for `session.ended`.
- **Problem / evidence:** `endTvSession` closes every HTTP response without checking its status, and the caller suppresses network exceptions without storing the queue promised by its comment. A 500 or network outage at expiry therefore leaves no retry, no local stop and potentially an open server session; there is no periodic server TV-session expiry job in the application startup paths.
- **Recommended fix:** Check the HTTP status, stop local gameplay/return HDMI control at the cutoff, and persist/retry the server end until confirmed. Reconcile on reconnect and test failed HTTP responses plus a dropped connection at zero time.
- **If we skip it:** A failed request at expiry can leave the console session running and the station stuck on the session screen.

### 17. P2 — WebSocket queues are unlimited and sibling tasks are not cancelled

**Location:** [`apps/backend/src/realtime/connection.rs:43`](apps/backend/src/realtime/connection.rs#L43), [`realtime/connection.rs:80`](apps/backend/src/realtime/connection.rs#L80), [`realtime/connection.rs:122`](apps/backend/src/realtime/connection.rs#L122).

- **What this is:** Each socket starts separate reader/writer tasks and an unbounded queue. The outer task waits for either task to finish and removes the connection from the registry.
- **Problem / evidence:** A slow peer can accumulate arbitrarily many queued event copies because the dispatcher never receives backpressure. When the writer ends first, the losing reader task is detached rather than cancelled and can retain its connection/database handles while awaiting a peer that does not finish closing; there is no read deadline.
- **Recommended fix:** Abort and join the sibling task when either loop finishes, release all senders, and use a bounded queue with a defined disconnect/resync policy for lagging peers. Test repeated disconnects and a client that stops reading.
- **If we skip it:** A slow client can exhaust a backend pod's memory, and half-open connections can retain tasks after removal from the connection registry.

### 18. P2 — Admin realtime is permanently disposed after effect cleanup

**Location:** [`apps/admin/src/lib/realtime/client.ts:37`](apps/admin/src/lib/realtime/client.ts#L37), [`lib/realtime/client.ts:134`](apps/admin/src/lib/realtime/client.ts#L134), [`lib/realtime/RealtimeProvider.tsx:31`](apps/admin/src/lib/realtime/RealtimeProvider.tsx#L31), [`main.tsx:20`](apps/admin/src/main.tsx#L20).

- **What this is:** The provider creates one client for its lifetime, connects it in an effect, and calls `disconnect` during cleanup. Disconnect permanently sets `disposed = true`; connect refuses a disposed client.
- **Problem / evidence:** React StrictMode's development setup/cleanup/setup sequence leaves the second setup unable to connect; any production rerun caused by token or role changes has the same problem. An in-memory harness using the actual transpiled class confirmed `connect → disconnect → connect` creates one socket instead of two.
- **Recommended fix:** Create a fresh client for each effect lifetime and expose that current instance through context, or explicitly distinguish reusable disconnect from permanent disposal. Add a provider lifecycle test covering effect replay and token replacement.
- **If we skip it:** The admin can silently stop receiving orders and session updates until the provider is fully remounted.

## Should fix

### 19. P2 — Backend and admin can deploy without their failing checks being run

**Location:** [`.github/workflows/deploy-production-backend.yml:39`](.github/workflows/deploy-production-backend.yml#L39), [`deploy-production-frontend.yml:34`](.github/workflows/deploy-production-frontend.yml#L34), [`apps/admin/package.json:9`](apps/admin/package.json#L9), [`apps/backend/tests/kiosk_session_flow.rs:42`](apps/backend/tests/kiosk_session_flow.rs#L42).

- **What this is:** The checked-in CI workflows test kiosk and TV paths; backend/admin deployment workflows build and publish without running their tests or admin typecheck. Database integration tests are explicitly ignored and often return early if environment or fixtures are absent.
- **Problem / evidence:** Admin typecheck currently fails with 26 diagnostics, including undefined notification words, table row typing and a boolean sent to a `0 | 1` filter; its Vite-only build does not enforce those types. Kiosk tests reproducibly fail at `src/lib/realtime.test.ts:75` (three reconnect logs versus one), and existing order tests check unauthenticated requests rather than successful sale conversion or concurrency.
- **Recommended fix:** Add required backend/admin checks, run database tests against a disposable migrated/seeded database with missing setup treated as a failure, and gate deployments on those checks. Fix the observed typecheck/test failures and make push filters include the active `master` branch and root build/lock/config changes as applicable.
- **If we skip it:** Passing image builds and partial test coverage can continue to ship broken access, billing and UI behavior.

### 20. P2 — Known dependency advisories remain, including an entirely unused Socket.IO stack

**Location:** [`pnpm-lock.yaml:2827`](pnpm-lock.yaml#L2827), [`pnpm-lock.yaml:2867`](pnpm-lock.yaml#L2867), [`packages/utils/package.json:47`](packages/utils/package.json#L47), [`packages/utils/src/lib/websockets/index.ts:1`](packages/utils/src/lib/websockets/index.ts#L1).

- **What this is:** The package audit reports 91 vulnerability counts across 88 advisory entries: 4 critical, 42 high, 38 moderate and 7 low. These are scanner results, not 91 demonstrated exploits in the shipped application.
- **Problem / evidence:** The lockfile includes Vitest 3.2.4 and Vite 7.3.3; their maintainers document conditional Windows/UI-server and network-exposed Windows dev-server risks ([Vitest advisory](https://github.com/vitest-dev/vitest/security/advisories/GHSA-5xrq-8626-4rwp), [Vite advisory](https://github.com/vitejs/vite/security/advisories/GHSA-fx2h-pf6j-xcff)). Whole-tree searches found that the 260-line Socket.IO helper has no consumers or public barrel export, yet its sole dependency remains installed and brings parser/transport advisories unrelated to the actual native WebSocket protocol.
- **Recommended fix:** Delete that unused helper and remove its `socket.io-client` dependency, then update the remaining affected direct/transitive packages using compatible patched versions and rerun tests/audit. Prioritize reachable development/CI exposure; the checked-in test commands use `vitest run`, so the critical UI-server advisory is not proof of production remote code execution.
- **If we skip it:** Developers retain avoidable dependency exposure, and reviewers must maintain an unused second realtime implementation.

## Prioritized remediation plan

| Order | Findings | Smallest complete work package | Evidence required before release |
| --- | --- | --- | --- |
| Immediate | 1, 2, 4 | Contain the account export; enforce role/ownership checks on user changes and sensitive reads. | Synthetic fixture only; negative HTTP authorization matrix; staff promotion attempt denied. |
| Next | 3, 5, 6 | Constrain legacy pairing/SSO, revoke existing privileged sessions when account state changes, and throttle public authentication. | Pairing cannot receive full operator credentials; revoked JWT/socket denied; concurrent attempt limit enforced. |
| Before further billing changes | 7–11, 13, 14 | Pass one database transaction through each business write; lock relevant session/account/order rows and enforce unique effects. Use the existing repository transaction pattern rather than adding a new service layer. | Concurrent and failure-injection tests prove one debit/grant/sale, rollback without partial writes, and matching payment totals. |
| Before relying on offline recovery | 12, 15, 16 | Preserve the real stop time, reconcile devices after reconnect, and make expiry locally effective with durable server retry. | Network-loss test at logout/expiry; reconnect after a remote end; no extra charge for stopped time. |
| Same repair cycle | 17, 18 | Correct socket task ownership, bound queues, and fix provider/client lifetime. | Repeated disconnects leave no tasks; slow reader is bounded; effect replay reconnects. |
| Release gate | 19, 20 | Fix the failing checks, add backend/admin/database gates, remove the unused Socket.IO code and refresh affected dependencies. | Admin/kiosk typechecks and tests pass; required database tests run; new audit results are triaged by actual exposure. |

For finding 1, history rewriting and credential resets are proposed remediation only. They were not performed. For finding 3, keep compatibility work explicit: the repository documents legacy endpoint retention, and external fleet usage cannot be inferred from current in-tree callers alone.

## Audit coverage summary

The tracked tree contains 1,117 files. Code Review Graph was used first with an explicit repository root; its current-commit graph reported 5,138 nodes, 42,019 edges and 746 indexed files. Architecture/community searches, auth/launch symbol searches, caller queries, affected-flow queries and `tests_for` queries narrowed the review. The Rust relationship results missed an obvious source-level SSO handler call and reported no affected flows for the selected billing/realtime files; source and test inspection therefore took precedence over graph absence or inferred coverage.

| Area | Reviewed | Depth and limits |
| --- | --- | --- |
| Architecture and boundaries | Root README, requirements/architecture/contributing docs, workspace/package manifests, backend composition/router, admin composition, kiosk/TV entry points and shared package imports. | The layered monolith fits a single venue. No runtime package-to-app import was found by the boundary search; the material boundary issue is failure to share database transactions across business effects. Architecture docs contain older stack/layout descriptions, so source was authoritative. |
| Authentication and authorization | HTTP middleware/extractors, JWT generation, password/TOTP checks, account updates/reset, device provisioning/pairing, legacy SSO, realtime ACL and sensitive read handlers. | Traced the principal trust boundaries. No live credential use, password cracking or production exploitation; all account-export inspection was aggregate-only. |
| Money and stored state | Sale creation/completion, product stock deduction, plan wallet recharge and ledger, credit eligibility/settlement, cash deposits/registers, expense approval and shift policy. | Read the main sale/session flows end to end and relevant repository/migration constraints; sampled auxiliary cash/expense/shift branches. No live SQL, database mutation, migration execution or query-plan measurement. |
| Sessions and recovery | Start/resume, heartbeat, server charging, end, kiosk queue/replay, logout cleanup, server events, TV bootstrap/countdown/auto-end. | Checked caller disagreement, retries and concurrent interleavings in source. Distributed failure cases require the disposable-database/network tests listed above. |
| Inventory | Stock receipt/adjustment/transfer/waste service paths and repository transaction/locking sections, order-backed POS. | Stock deductions recheck locked stock and enforce nonnegative quantities; duplicate line validation alone was not reported as an oversell because the later locked deduction protects it. Did not read every inventory/report query. |
| Performance and memory | Socket queues/tasks and dispatcher DB waits, cache-aside/error paths and prefix invalidation, blocking bcrypt, native launch/cleanup boundaries. | No large-scale benchmark, heap profile or database `EXPLAIN`; no speculative need for microservices or new caching layers. |
| Admin and shared UI | HTTP/error handling, auth session handling, realtime provider/client, notification formatting, payment split validation, selected service/DTO callers, shared exports and tests. | Static and unit-test review; not an exhaustive visual/accessibility review of every page or component. |
| Windows kiosk | App/native entry point, session provider and offline queue, CSP/capabilities, token persistence, allow-list launch gates/Windows argument quoting, idle cleanup generation checks, selected startup/recovery and updater configuration. | No real Windows lockdown, game-process, installer, UAC, updater-signature or WebView2 exercise. The large process/window-management module was sampled, not read in full. |
| Android TV | API/token/realtime clients, ViewModel session lifecycle, build/signing setup, CI/release workflows and clock test presence. | Source review; no Android build, emulator, physical TV, HDMI-CEC or network fault exercise. |
| CI/CD and configuration | All eight workflow files, Dockerfiles, Turbo config, Helm backend/admin/frontend/Postgres/Redis values and selected templates, migrations/scripts, Node/Rust manifests and lockfile versions. | No image build, Helm render, cluster inspection, restore drill, environment secret access or actual branch-protection verification. `helm` and `cargo-audit` were unavailable. |
| Debt, duplication and dependencies | Legacy Socket.IO consumers, shared exports, deprecated SSO compatibility references, build/test scripts, package audit and two primary advisories. | Confirmed unused Socket.IO code by whole-tree searches including config/tests/docs; did not equate public compatibility endpoints with dead code. No wholesale dependency update or full Rust/Gradle advisory audit. |

### Checks actually run

| Check | Result |
| --- | --- |
| `pnpm -r --if-present test` | Failed overall: utils **38/38**, UI **11/11**, admin **25/25** passed; kiosk **159/160**, with the reconnect-log test failing. |
| `pnpm --filter @gaming-cafe/kiosk exec vitest run src/lib/realtime.test.ts` | Independently reproduced the same failure; **3 passed, 1 failed**. |
| `pnpm --filter @gaming-cafe/admin typecheck` | Failed with **26 TypeScript diagnostics** across six files; examples and affected behavior are in finding 19. |
| `pnpm --filter @gaming-cafe/kiosk typecheck` | Passed. |
| `cargo test --locked --offline --manifest-path apps/backend/Cargo.toml --lib` | **66 passed**, no failures or ignored library tests; five compiler warnings. This did not run integration test binaries. |
| Admin client in-memory lifecycle harness | Confirmed one socket creation for `connect → disconnect → connect`, where reconnection requires a second. Used the actual TypeScript class transpiled in memory, with fake storage/socket; no file edits. |
| `pnpm audit --json` | Nonzero vulnerability result; metadata counts **4 critical / 42 high / 38 moderate / 7 low**. JSON retained in `/tmp/arena360-pnpm-audit.json` for this local session; it contains dependency advisories, not application credentials. |
| Read-only export inspection | Parsed CSV headers/counts and bcrypt prefixes; no account values printed or tested. |

Observed tools: Node 22.22.3, pnpm 9.15.9 and Cargo 1.95.0. CI uses Node 20, so a CI run remains necessary after remediation. Two lower-priority confirmed issues were left out of the numbered findings to honor the 20-finding limit; individual dependency advisories are grouped in finding 20 rather than presented as separate application defects.

**Verdict:** Fix account-data exposure and authorization first, then make billing/session/order effects atomic before treating financial totals or offline recovery as reliable.

**Lean: -260 lines, -1 direct dependency possible.** This counts only the verified-unused Socket.IO helper; lockfile/transitive removal savings are additional and were not estimated. No new framework or architectural split is justified by these findings.

**Not checked:** Live database/Redis behavior, ignored integration suites, deployed access controls and secret validity, backups/restores, full Git history, Windows/Android runtime and hardware, release artifacts, all page layouts, every auxiliary query/script branch, and Rust/Gradle vulnerability databases. Source-derived races and access paths remain unpatched risks until remediation and regression checks are completed.
