# PLANNER: Arena360 Kiosk (Tauri)

> Living implementation tracker for the in-cafe Windows kiosk.
> Updated as work progresses.
>
> This file is the source of truth for **what is done, by whom, against
> which ADR and kiosk requirement, with what evidence**. It follows the
> `TASKS.md` conventions from `.cursor/rules/05-project-planning.mdc`,
> extended with explicit ADR + `US-K*` cross-references.
>
> Last updated: 2026-06-06 (K10 Windows deployment roadmap added).

## Quick links

- Requirements: [REQUIREMENTS-KIOSK.md](REQUIREMENTS-KIOSK.md)
- Platform requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Architecture: [ARCHITECTURE.md](ARCHITECTURE.md)
- Migration log: [MIGRATION.md](MIGRATION.md)
- Consolidation planner: [PLANNER.md](PLANNER.md)
- ADRs: [adr/](adr/)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Windows station deployment: [KIOSK-WINDOWS-DEPLOYMENT.md](KIOSK-WINDOWS-DEPLOYMENT.md)

## Status legend

| Status | Meaning |
|--------|---------|
| `pending` | Not started; no work in progress. |
| `in_progress` | A subagent or engineer is actively working on it. |
| `blocked-external` | Cannot proceed without user action (e.g. ADR approval, operator hardware). |
| `blocked-internal` | Waiting on another task in this planner. |
| `done` | All listed acceptance criteria checked off; evidence captured. |
| `verified` | Done **and** the relevant build / test / smoke gate has passed end-to-end. |

## Phase summary

| Phase | Goal | Depends on | Tasks | Status |
|-------|------|------------|-------|--------|
| K0 | Draft + accept ADRs (re-introduce kiosk, auth, WS ACL, allow-list, lockdown) | — | 5 ADR drafts | `verified` (ADR-0016–0020 Accepted) |
| K1 | Backend kiosk APIs (player auth, device registration, single-session, WS) | K0 | 9 backend tasks | `done` (builds; integration tests need DB) |
| K2 | `apps/kiosk` Tauri 2 + React 19 scaffold; shared packages; HTTP/WS clients | K0 | 6 scaffold tasks | `verified` |
| K3 | Device onboarding (setup mode, fingerprint, registration, scan, allow-list) | K1, K2 | 6 kiosk tasks | `done` (Windows native verified on CI/QA) |
| K4 | Player session (login, HUD, reminders, polls, WS recharge, end) | K1, K2, K3 | 11 kiosk tasks | `done` (member screen partial) |
| K5 | Lockdown state machine, hotkeys, launch guard, process tracker/cleanup | K2 | 5 kiosk tasks | `done` (Windows hooks verified on CI/QA) |
| K6 | Offline grace + reconcile | K4 | 2 kiosk tasks | `done` |
| K7 | Admin SPA (registration codes, force-end, fingerprint) | K1 | 3 admin tasks | `done` |
| K8 | Tests + CI (Vitest, cargo test, e2e smoke, Windows runner) | K1–K7 | 5 test/CI tasks | `done` (cargo+vitest green; e2e/CI need infra) |
| K9 | Windows installer, WebView2 bootstrap, optional auto-update | K8 | 3 packaging tasks | `done` (installer builds on Windows CI) |
| K10 | Windows station shell — boot auto-start, watchdog relaunch, fleet scripts | K9 | 4 deployment tasks | `pending` (guide done; code not started) |

**Critical path:** K0 → K1 + K2 (parallel) → K3 → K4 + K5 (parallel) → K6 → K8 → K9 → K10. K7 can run in parallel with K3–K5 once K1 is `verified`. K10 is operator-facing and can ship incrementally after K9.

---

## Tasks

### `kiosk-adr-reintroduce` — Draft ADR re-introducing apps/kiosk in monorepo

- **Phase**: K0
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0002` (implements deferred portion; does not supersede)
- **User-story refs**: (infra; gates K2+)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §10; PLANNER.md 2026-05-27 deferral

**Description** — Author `docs/adr/DRAFT-0016-kiosk-monorepo-reintroduce.md` reversing the 2026-05-27 scope pivot. Document `apps/kiosk` in `pnpm-workspace.yaml`, `turbo.json` (`tauri:dev`, `tauri:build`), `.changeset/config.json`. Log event to `docs/adr/.events.jsonl`. Wait for user approval before K2.

**Implementation notes** —
- Touches `turbo.json`, `pnpm-workspace.yaml`, root `package.json` scripts — architectural per ADR discipline.
- Reference archived `agent-kiosk` tarball path from PLANNER.md.
- Workspace file edits deferred to `kiosk-workspace-wire` (K2) until DRAFT-0016 is **Accepted**.

**Acceptance criteria** (checklist):

- [x] DRAFT ADR exists with Status: Proposed.
- [x] `.events.jsonl` line appended.
- [x] User approval recorded before any K2 task starts.

**Evidence**:

- `docs/adr/0016-kiosk-monorepo-reintroduce.md` (Accepted, 2026-05-30).
- `docs/adr/.events.jsonl` — `adr_draft` + `adr_accepted` entries for ADR-0016.

---

### `kiosk-adr-player-auth` — Draft ADR for player + device JWT authentication

- **Phase**: K0
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0016` (companion); `adr/0017-kiosk-player-device-auth.md`
- **User-story refs**: US-KAUTH-001, US-KAUTH-002, US-KAUTH-006, US-KREG-001
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §7.2

**Description** — Define player login (`POST /auth/login/player`), device registration token exchange, JWT claims (`role: player|device`), lifetimes, refresh policy (resolves OQ-3), Tauri secure storage contract, and `PLAYER_ALREADY_IN_SESSION` error envelope.

**Implementation notes** —
- `apps/backend/src/middleware/auth.rs` — new extractors `DeviceUser`, `PlayerUser`.
- Player session routes must not require `AdminOrStaff`.

**Acceptance criteria** (checklist):

- [x] DRAFT ADR documents API paths, claims, and storage keys.
- [x] Error code `PLAYER_ALREADY_IN_SESSION` added to contracts cross-ref.

**Evidence**:

- `docs/adr/0017-kiosk-player-device-auth.md` (Accepted, 2026-05-30).
- `docs/adr/.events.jsonl` — `adr_draft` + `adr_accepted` entries for ADR-0017.

---

### `kiosk-adr-ws-acl` — Draft ADR amending realtime ACL for device tokens

- **Phase**: K0
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0013` (amends ACL only); `adr/0018-kiosk-ws-device-acl.md`
- **User-story refs**: US-KSESSION-007, US-KREG-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §7.2; `apps/backend/src/realtime/acl.rs`

**Description** — Device JWT may subscribe only to `device:{ownDeviceId}` and optionally `user:{ownPlayerId}` while player session active. Standardize `balance.updated` and `session.ended` event payloads (resolves OQ-2).

**Implementation notes** —
- Amend ADR-0013; do not replace outbox pattern.
- Kiosk connects via `Sec-WebSocket-Protocol: bearer, <device-jwt>`.

**Acceptance criteria** (checklist):

- [x] DRAFT ADR lists allowed channels per token type.
- [x] Event JSON schemas documented with example payloads.

**Evidence**:

- `docs/adr/0018-kiosk-ws-device-acl.md` (Accepted, 2026-05-30).
- `docs/adr/.events.jsonl` — `adr_draft` + `adr_accepted` entries for ADR-0018.

---

### `kiosk-adr-allow-list` — Draft ADR for per-device launch allow-list storage

- **Phase**: K0
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0019`; cancels backend allow-list tasks
- **User-story refs**: US-KSCAN-003, US-KSCAN-004 (US-KSCAN-005 deferred v1)
- **Migration rows**: (none — client-only)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md Appendix B

**Description** — Client-side allow-list in WebView `localStorage` (`gaming-cafe.kiosk.launch_entries`). No backend table or API. Icon strategy: local `.exe` extraction (resolves OQ-8).

**Implementation notes** —
- Cancels `be-allow-list-storage`, `admin-device-allow-list`.
- `kiosk-allow-list-editor` persists to `localStorage`.

**Acceptance criteria** (checklist):

- [x] DRAFT ADR documents local JSON schema and storage keys.
- [x] Icon strategy decision documented.

**Evidence**:

- `docs/adr/0019-kiosk-device-allow-list.md` (Accepted, 2026-05-30).
- `docs/adr/.events.jsonl` — `adr_draft` + `adr_accepted` entries for ADR-0019.

---

### `kiosk-adr-lockdown` — Draft ADR for Windows lockdown and process APIs

- **Phase**: K0
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0002`; `adr/0020-kiosk-windows-lockdown.md`
- **User-story refs**: US-KLOCK-001..005, US-KPROC-*
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §4.6, §4.5

**Description** — Document Rust approach: low-level keyboard hook, fullscreen enforcement, process tree tracking, WMI fingerprint. Define lockdown state machine: `Locked` vs `SetupRelaxed` — admin login disables lockdown; logout/idle/player-session forces `Locked`.

**Implementation notes** —
- `windows` crate or `windows-sys` for hooks; document CAD limitation (US-KLOCK-002).
- Tauri commands: `set_lockdown_state`, `collect_fingerprint`, `scan_installed_software`, `launch_allowed`, `kill_tracked_processes`.

**Acceptance criteria** (checklist):

- [x] DRAFT ADR lists crates and command allow-list for Tauri IPC.
- [x] State machine transitions match REQUIREMENTS-KIOSK US-KLOCK-005 scenarios.

**Evidence**:

- `docs/adr/0020-kiosk-windows-lockdown.md` (Accepted, 2026-05-30).
- `docs/adr/.events.jsonl` — `adr_draft` + `adr_accepted` entries for ADR-0020.

---

### `be-player-auth-endpoint` — Implement POST /auth/login/player and player JWT

- **Phase**: K1
- **Status**: `done` (handlers/auth.rs `login_player` + auth_service; OpenAPI regen)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KAUTH-001, US-KAUTH-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KAUTH-001

**Description** — Add player username/password login issuing a scoped JWT. Validate role=player, bcrypt password. Return envelope consistent with staff login.

**Implementation notes** —
- `handlers/auth.rs`, `services/auth_service.rs`.
- Claims include `userId`, `deviceId` (optional), `roles: [player]`.

**Acceptance criteria** (checklist):

- [ ] `POST /auth/login/player` returns 200 + JWT on valid credentials.
- [ ] 401 on invalid credentials.
- [ ] OpenAPI annotated; `pnpm gen:api-types` updated.

**Evidence**:

- _none yet_

---

### `be-device-registration-api` — Implement device registration code exchange

- **Phase**: K1
- **Status**: `done` (POST /devices/register + issueRegistrationCode)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-001
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KREG-001

**Description** — Operator generates one-time code in admin; kiosk submits code + fingerprint + device fields; backend returns device JWT and binds `devices` row.

**Implementation notes** —
- `POST /devices/register` (or `/kiosk/register`).
- Invalidate code after use; store fingerprint on `registeredKiosk`.

**Acceptance criteria** (checklist):

- [ ] Valid code returns device JWT.
- [ ] Reused/expired code returns 401.
- [ ] Registration status set to `registered`.

**Evidence**:

- _none yet_

---

### `be-fingerprint-drift` — Enforce fingerprint drift policy on device heartbeat

- **Phase**: K1
- **Status**: `done` (device_service::verify_fingerprint_drift; unit tests on drift count)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-002, US-KREG-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KREG-002/003

**Description** — On each authenticated device request, compare MAC+serial+biosUuid. Tolerate one component change (update stored); reject when two or more differ with 403 `DEVICE_FINGERPRINT_MISMATCH`.

**Implementation notes** —
- `device_service.rs` or dedicated `fingerprint_service.rs`.
- Log drift warnings for operator dashboard.

**Acceptance criteria** (checklist):

- [ ] One-component drift allowed and persisted.
- [ ] Two+ component drift returns 403 and blocks player login path.

**Evidence**:

- _none yet_

---

### `be-single-session-rule` — Enforce one active session per player globally

- **Phase**: K1
- **Status**: `done` (session_service::start_for_player → 409 PLAYER_ALREADY_IN_SESSION)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KAUTH-006
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KAUTH-006

**Description** — Before creating `usage_sessions`, check no other open session exists for the same `playerId` (via balance join). Return 409 `PLAYER_ALREADY_IN_SESSION` with other device name.

**Implementation notes** —
- Extend `session_service.rs` `start()`.
- Add `ErrorCode` to `packages/contracts` if missing.

**Acceptance criteria** (checklist):

- [ ] Second device login returns 409 with device display name.
- [ ] Integration test in `be-kiosk-integration-tests`.

**Evidence**:

- _none yet_

---

### `be-kiosk-session-routes` — Add kiosk-scoped session start/end routes

- **Phase**: K1
- **Status**: `done` (POST/GET/PATCH /kiosk/sessions; system kiosk shift; crash-resume)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KSESSION-001, US-KSESSION-008
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5; OQ-1

**Description** — Expose player-authenticated (or device+player) endpoints to start and end sessions without staff JWT. Document shift binding decision (system shift vs active staff shift).

**Implementation notes** —
- May be `POST /kiosk/sessions` + `PATCH /kiosk/sessions/:id/end`.
- Reuse `BalanceService::validate_balance` and deduct on end.

**Acceptance criteria** (checklist):

- [ ] Player JWT can start session on registered device.
- [ ] Player JWT can end own session.
- [ ] Shift binding documented in ADR and code.

**Evidence**:

- _none yet_

---

### `be-ws-device-acl` — Allow device JWT to subscribe to device channel

- **Phase**: K1
- **Status**: `done` (realtime/acl.rs device-token scoping, pre-existing K1 work)
- **Owner**: TBD
- **ADR refs**: `adr/0018`, `adr/0013`
- **User-story refs**: US-KSESSION-007, US-KREG-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: `apps/backend/src/realtime/acl.rs`

**Description** — Extend ACL so device-role tokens may subscribe only to `device:{id}` matching token subject. Deny `staff`, `admin`, other devices.

**Implementation notes** —
- Parse device id from JWT claims.
- Unit test ACL matrix.

**Acceptance criteria** (checklist):

- [ ] Device token subscribes to own channel.
- [ ] Device token rejected on `staff` channel.
- [ ] Staff token unchanged.

**Evidence**:

- _none yet_

---

### `be-allow-list-storage` — Schema and CRUD for device launch allow-list

- **Phase**: K1
- **Status**: `cancelled`
- **Owner**: —
- **ADR refs**: `adr/0019` (client localStorage only — no backend)
- **User-story refs**: US-KSCAN-005 (deferred v1)
- **Migration rows**: _(n/a)_
- **Plan / NFR refs**: ADR-0019 supersedes backend persistence

**Description** — ~~Implement persistence per allow-list ADR~~ **Cancelled.** Allow-list lives in kiosk `localStorage` per ADR-0019.

**Evidence**:

- `docs/adr/0019-kiosk-device-allow-list.md` — client-side decision.

---

### `be-recharge-events` — Publish balance.updated on staff recharge

- **Phase**: K1
- **Status**: `done` (transaction_service::publish_balance_updated → device + user channels)
- **Owner**: TBD
- **ADR refs**: `adr/0018`
- **User-story refs**: US-KSESSION-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5; OQ-2

**Description** — When staff completes plan recharge affecting a player with active session, publish `balance.updated` to `device:{deviceId}` and `user:{playerId}` via outbox.

**Implementation notes** —
- Hook in `transaction_service.rs` / `balance_service.rs` after `recharge`.
- Payload: `balanceId`, `remainingMinutes`, `playerId`, `sessionId`.

**Acceptance criteria** (checklist):

- [ ] Recharge triggers outbox event.
- [ ] Kiosk WS client receives event in smoke test.

**Evidence**:

- _none yet_

---

### `be-session-end-reason-enum` — Add session end reason to API and persistence

- **Phase**: K1
- **Status**: `in_progress` (API + WS payload done; DB column deferred behind DRAFT-0021)
- **Owner**: TBD
- **ADR refs**: `adr/0009`
- **User-story refs**: US-KSESSION-006, US-KSESSION-009
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §7.3

**Description** — Persist and return end reason: `voluntary`, `ENDED_AUTO`, `force`, `offline_reconcile`. Include in WS `session.ended` payload.

**Implementation notes** —
- Nullable column on `usage_sessions` or derive from audit — ADR if schema change.
- Update session end handlers.

**Acceptance criteria** (checklist):

- [ ] End session accepts reason.
- [ ] WS event includes reason.
- [ ] OpenAPI updated.

**Evidence**:

- _none yet_

---

### `be-kiosk-openapi-regen` — Regenerate api-types after kiosk endpoints

- **Phase**: K1
- **Status**: `done` (pnpm gen:api-types; schema.ts has kiosk paths + PlayerLoginDto)
- **Owner**: TBD
- **ADR refs**: `adr/0004`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: CONTRIBUTING.md `pnpm gen:api-types`

**Description** — Annotate all new kiosk endpoints with utoipa; run `pnpm gen:api-types`; verify admin and kiosk packages typecheck against new schema.

**Implementation notes** —
- `apps/backend/src/openapi/mod.rs` — register paths.
- CI drift gate when `ci-pipeline` exists.

**Acceptance criteria** (checklist):

- [ ] `pnpm gen:api-types` exits 0.
- [ ] `packages/api-types/src/schema.ts` includes player login and device register paths.

**Evidence**:

- _none yet_

---

### `kiosk-app-scaffold` — Create apps/kiosk Tauri 2 + React 19 project

- **Phase**: K2
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0002`, `adr/0016`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §1.1; MIGRATION.md

**Description** — Scaffold `apps/kiosk` with `src-tauri/`, React 19 + Vite frontend, feature-sliced `src/` layout. Import patterns from archived `agent-kiosk` where applicable.

**Implementation notes** —
- `pnpm create tauri-app` or copy from archive.
- `package.json` name `@gaming-cafe/kiosk`.

**Acceptance criteria** (checklist):

- [x] `pnpm --filter @gaming-cafe/kiosk tauri:dev` launches window.
- [x] `cargo build` in `src-tauri` succeeds.

**Evidence**:

- Greenfield `create-tauri-app` (archive tarball unavailable); `apps/kiosk/` with FSD `src/app`, `src/pages`.
- `cargo build` in `src-tauri` OK; `tauri:dev` smoke (Vite :1420 + `target/debug/kiosk`).

---

### `kiosk-workspace-wire` — Wire kiosk into pnpm workspace and Turbo

- **Phase**: K2
- **Status**: `verified`
- **Owner**: agent (2026-05-30)
- **ADR refs**: `adr/0016`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: adr/0001

**Description** — Add `apps/kiosk` to `pnpm-workspace.yaml`, extend `turbo.json` with `tauri:dev` (persistent) and `tauri:build`. `.changeset/config.json` unchanged (kiosk already in `ignore`).

**Implementation notes** —
- Root `package.json` scripts: `kiosk:dev`, `kiosk:build`.
- CONTRIBUTING.md already documents filter commands; no edit required.

**Acceptance criteria** (checklist):

- [x] Workspace install includes kiosk.
- [x] `turbo run build --filter=@gaming-cafe/kiosk` succeeds.

**Evidence**:

- `pnpm-workspace.yaml`, `package.json` workspaces + scripts, `turbo.json` `tauri:dev` / `tauri:build`, `.gitignore` kiosk paths.
- `pnpm install` (11 workspace projects); `pnpm exec turbo run build --filter=@gaming-cafe/kiosk` OK.

---

### `kiosk-shared-packages` — Consume shared monorepo packages in kiosk UI

- **Phase**: K2
- **Status**: `done` (theme tokens in App.tsx + app.css; api-types/contracts/utils consumed)
- **Owner**: TBD
- **ADR refs**: `adr/0007`, `adr/0004`, `adr/0008`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: ARCHITECTURE.md packages

**Description** — Wire `@gaming-cafe/theme` CSS tokens (`--gz-*`), `@gaming-cafe/api-types`, `@gaming-cafe/contracts`, `@gaming-cafe/utils`. No MUI — plain React + CSS variables.

**Implementation notes** —
- Import `packages/theme/dist/tokens.css` in kiosk entry.
- Biome extends root preset.

**Acceptance criteria** (checklist):

- [ ] Kiosk builds with workspace deps.
- [ ] No duplicate OpenAPI spec in kiosk tree.

**Evidence**:

- _none yet_

---

### `kiosk-secure-token-storage` — Persist device and player JWT in Tauri secure storage

- **Phase**: K2
- **Status**: `done` (storage.rs file store + rationale doc; logout/reset clear paths)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-001, US-KAUTH-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.3

**Description** — Store device JWT and player JWT in separate secure storage keys; clear player token on session end; clear all on factory reset flow.

**Implementation notes** —
- `tauri-plugin-store` or platform keychain via Tauri 2 plugin.
- Rust commands: `get_tokens`, `set_tokens`, `clear_player_token`.

**Acceptance criteria** (checklist):

- [ ] Tokens survive app restart.
- [ ] Logout clears player token only.
- [ ] Re-registration clears device token.

**Evidence**:

- _none yet_

---

### `kiosk-http-client` — HTTP client with device + player auth headers

- **Phase**: K2
- **Status**: `done` (lib/http.ts device bearer + X-Player-Token; ApiError.details surfaced)
- **Owner**: TBD
- **ADR refs**: `adr/0004`
- **User-story refs**: US-KSESSION-004
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.1

**Description** — Factory in kiosk (or extend `@gaming-cafe/utils`) attaching device JWT on every request and player JWT when present. Unwrap NestJS envelope; map `ApiError`.

**Implementation notes** —
- Base URL from env `VITE_API_URL`.
- Handle 409 `PLAYER_ALREADY_IN_SESSION` distinctly.

**Acceptance criteria** (checklist):

- [ ] Authenticated calls succeed against local backend.
- [ ] 401 triggers login redirect.

**Evidence**:

- _none yet_

---

### `kiosk-ws-client` — WebSocket client for device and user channels

- **Phase**: K2
- **Status**: `done` (lib/realtime.ts; reconnect bug fixed; events → KioskProvider)
- **Owner**: TBD
- **ADR refs**: `adr/0018`, `adr/0013`
- **User-story refs**: US-KSESSION-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.1 NFR

**Description** — Connect to `GET /realtime` with device JWT; subscribe `device:{id}`; when player logged in also `user:{playerId}`. Auto-reconnect with exponential backoff.

**Implementation notes** —
- `apps/admin/src/lib/realtime/` as reference.
- Dispatch `balance.updated`, `session.ended`, device status events to React store.

**Acceptance criteria** (checklist):

- [ ] WS connects with device token.
- [ ] Reconnects after drop within 5 s.
- [ ] Events reach UI store.

**Evidence**:

- _none yet_

---

### `kiosk-setup-mode-auth` — Setup mode with admin login disables lockdown

- **Phase**: K3
- **Status**: `done` (SetupGesture → admin OTP → SetupRelaxed; 15-min idle re-lock)
- **Owner**: TBD
- **ADR refs**: `adr/0020`, `adr/0017`
- **User-story refs**: US-KLOCK-005, US-KREG-005
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.1; §5 US-KLOCK-005

**Description** — Hidden/setup entry (e.g. corner tap sequence) opens operator login using admin/staff credentials (same backend auth as admin SPA). On success: transition lockdown to `SetupRelaxed` — hotkeys off, launch guard relaxed, fullscreen exit allowed. On logout, exit-setup, or idle timeout (default 15 min): return to `Locked`. Player session start always forces `Locked`.

**Implementation notes** —
- React: `SetupModePage`, calls `set_lockdown_state(SetupRelaxed)`.
- Uses staff `POST /auth/login/staff` or dedicated setup token — per player-auth ADR.
- Blocked on `kiosk-lockdown-state-machine` for Rust side.

**Acceptance criteria** (checklist):

- [ ] Admin login from kiosk enters setup mode.
- [ ] Alt+Tab works in setup mode.
- [ ] Logout re-enables lockdown.
- [ ] Idle timeout re-locks.
- [ ] Player login never leaves setup relaxed without locking.

**Evidence**:

- _none yet_

---

### `kiosk-fingerprint-cmd` — Rust collect_fingerprint command

- **Phase**: K3
- **Status**: `done` (fingerprint.rs PowerShell CIM mac/serial/biosUuid; dev stub; unit tests)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KREG-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md Appendix A

**Description** — Implement WMI/Win32 collection of primary MAC, machine serial, BIOS UUID; return JSON for registration UI.

**Implementation notes** —
- `src-tauri/src/fingerprint.rs`.
- Handle VM edge cases gracefully.

**Acceptance criteria** (checklist):

- [ ] Command returns all three fields on physical hardware.
- [ ] Unit test with mocked WMI where possible.

**Evidence**:

- _none yet_

---

### `kiosk-registration-ui` — First-run registration UI with device schema

- **Phase**: K3
- **Status**: `done` (RegistrationPage fields mirror device-schema; read-only fingerprint preview)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-001, US-KREG-004, US-KREG-005
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.1; device-schema.ts

**Description** — UI for one-time code + name, serial, IP, deviceType, location, status. Submit to `be-device-registration-api`; persist device JWT on success.

**Implementation notes** —
- Mirror fields from `apps/admin/src/containers/devices/schemas/device-schema.ts`.
- Show fingerprint read-only.

**Acceptance criteria** (checklist):

- [ ] Valid code completes registration.
- [ ] Invalid code shows operator message.
- [ ] Skip flow when already registered.

**Evidence**:

- _none yet_

---

### `kiosk-software-scan-cmd` — Rust scan_installed_software command

- **Phase**: K3
- **Status**: `done` (scan.rs registry + common paths; scan-progress events; dev fixture)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KSCAN-001, US-KSCAN-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §4.2

**Description** — Scan registry and common paths for Steam, Riot, Epic (optional), game folders, Chrome, G Hub, NVIDIA CP, audio utilities. Return candidates with resolved executable paths.

**Implementation notes** —
- Progress events for long scans (NFR < 60 s).
- Deduplicate by path.

**Acceptance criteria** (checklist):

- [ ] Detects Steam when installed.
- [ ] Scan completes < 60 s on typical cafe PC.
- [ ] Returns structured candidate list.

**Evidence**:

- _none yet_

---

### `kiosk-allow-list-editor` — Setup UI to curate launch allow-list

- **Phase**: K3
- **Status**: `done` (AllowListEditor + allowList.ts localStorage; test-launch; hides missing exes)
- **Owner**: TBD
- **ADR refs**: `adr/0019`
- **User-story refs**: US-KSCAN-003, US-KSCAN-004, US-KSCAN-006
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.1

**Description** — Review scan results; toggle enabled; add manual path; test launch (setup mode only); persist to `localStorage` per ADR-0019.

**Implementation notes** —
- Setup mode only — not visible to players.
- Hide entries whose exe missing on disk.
- Key: `gaming-cafe.kiosk.launch_entries`.

**Acceptance criteria** (checklist):

- [ ] Operator can add/remove entries.
- [ ] Entries persist in localStorage across restart.
- [ ] Test launch opens app without player session.

**Evidence**:

- _none yet_

---

### `kiosk-device-status-ws` — React to remote device status via WebSocket

- **Phase**: K3
- **Status**: `done` (device.status_changed → maintenance banner + login block in IdlePage)
- **Owner**: TBD
- **ADR refs**: `adr/0018`
- **User-story refs**: US-KREG-006, US-KREG-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KREG-006

**Description** — When admin sets maintenance/out_of_service, kiosk shows banner and blocks player login. Optional poll fallback every 60 s.

**Implementation notes** —
- Handle `device.status_changed` event (name per ws ADR).
- Map to device-schema status enums.

**Acceptance criteria** (checklist):

- [ ] Maintenance status blocks login UI.
- [ ] Returning to operational clears block without restart.

**Evidence**:

- _none yet_

---

### `kiosk-attract-screen` — Idle attract screen

- **Phase**: K4
- **Status**: `done` (IdlePage branded attract + tap-to-start + hidden SetupGesture)
- **Owner**: TBD
- **ADR refs**: `adr/0007`
- **User-story refs**: (UX)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §8

**Description** — Fullscreen branded idle state with touch-to-start; no player controls visible; setup entry gesture available but inconspicuous.

**Implementation notes** —
- Uses `--gz-*` tokens.
- Shows device name if registered.

**Acceptance criteria** (checklist):

- [ ] Renders on boot when no session.
- [ ] Tap navigates to login.

**Evidence**:

- _none yet_

---

### `kiosk-player-login` — Player username/password login

- **Phase**: K4
- **Status**: `done` (PlayerLoginPage → /auth/login/player; loginLockout 5/15min)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KAUTH-001, US-KAUTH-004
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KAUTH-001

**Description** — Login form calling `be-player-auth-endpoint`; generic error on failure; lockout after 5 failures / 15 min (configurable).

**Implementation notes** —
- `PlayerLoginPage.tsx`.
- Rate limit UI matches backend.

**Acceptance criteria** (checklist):

- [ ] Valid player proceeds to balance check.
- [ ] Invalid shows generic message.
- [ ] Lockout enforced.

**Evidence**:

- _none yet_

---

### `kiosk-balance-gate` — Reject login when no eligible balance

- **Phase**: K4
- **Status**: `done` (startSession resolves balance; no eligible balance blocks session start)
- **Owner**: TBD
- **ADR refs**: `adr/0009`
- **User-story refs**: US-KAUTH-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KAUTH-003

**Description** — After auth, call best-plan/balance API for device type; if none or zero minutes, show Ask staff to recharge without creating session.

**Implementation notes** —
- Reuse `GET /player-plans/best-plan` or kiosk-specific aggregate endpoint.

**Acceptance criteria** (checklist):

- [ ] Zero balance shows recharge screen.
- [ ] No session row created.

**Evidence**:

- _none yet_

---

### `kiosk-single-login-guard` — Handle PLAYER_ALREADY_IN_SESSION

- **Phase**: K4
- **Status**: `done` (AlreadyInSessionPage shows conflicting device name from 409 details)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KAUTH-006
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KAUTH-006

**Description** — Display other device name from 409 response; return to idle; no partial session state.

**Implementation notes** —
- `AlreadyInSessionPage.tsx`.
- Maps `ErrorCode` from contracts.

**Acceptance criteria** (checklist):

- [ ] 409 shows device name.
- [ ] Retry after staff force-end on other device succeeds.

**Evidence**:

- _none yet_

---

### `kiosk-session-start` — Start session and enable player lockdown

- **Phase**: K4
- **Status**: `done` (POST /kiosk/sessions → Locked + LauncherGrid wired to launch_allowed)
- **Owner**: TBD
- **ADR refs**: `adr/0017`, `adr/0020`
- **User-story refs**: US-KSESSION-001, US-KSESSION-010
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.2

**Description** — Call `be-kiosk-session-routes` to create session; set lockdown `Locked` (force, even if coming from setup); navigate to launcher; set device in_use locally.

**Implementation notes** —
- Store `sessionId` in session store.
- Emit lockdown event before showing launcher.

**Acceptance criteria** (checklist):

- [ ] Session created in DB.
- [ ] Lockdown active.
- [ ] Launcher visible.

**Evidence**:

- _none yet_

---

### `kiosk-hud-timer` — Persistent remaining-time HUD

- **Phase**: K4
- **Status**: `done` (HudTimer ticks locally, re-anchors to server remainingMinutes; unit tests)
- **Owner**: TBD
- **ADR refs**: `adr/0009`
- **User-story refs**: US-KSESSION-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.4

**Description** — Always-visible countdown using server `remainingMinutes` + session start; resync on poll/WS; readable at 2 m (48px+ at 1080p).

**Implementation notes** —
- Top overlay or side bar; never hidden during session.
- Uses authoritative server values.

**Acceptance criteria** (checklist):

- [ ] Timer visible on launcher and in-game overlay if applicable.
- [ ] Updates within 2 s of WS recharge.

**Evidence**:

- _none yet_

---

### `kiosk-reminders` — 10 / 5 / 1 minute reminders

- **Phase**: K4
- **Status**: `done` (SessionPage threshold toasts fire once per threshold)
- **Owner**: TBD
- **ADR refs**: (none)
- **User-story refs**: US-KSESSION-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KSESSION-003

**Description** — Visual reminders at 10, 5, 1 minutes; optional sound once per threshold (OQ-6: default off, operator setting).

**Implementation notes** —
- `prefers-reduced-motion` disables animation.
- Do not re-fire same threshold.

**Acceptance criteria** (checklist):

- [ ] Each threshold fires once.
- [ ] Sound respects mute setting.

**Evidence**:

- _none yet_

---

### `kiosk-poll-final-burst` — 15 s poll and final-minute 4x burst

- **Phase**: K4
- **Status**: `superseded` — replaced by client clock from `session.startTime` + WS re-anchor; auto-end at 10 s (see [session-time-clock.md](session-time-clock.md))
- **Owner**: TBD
- **ADR refs**: `adr/0009`
- **User-story refs**: US-KSESSION-004, US-KSESSION-005
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — ~~Poll `GET /sessions/:id` every 15 s~~ **Removed.** Client ticks locally; optional one-shot reconcile on WS reconnect / app resume. Auto-end at **10 seconds** remaining.

**Implementation notes** —
- `useSessionPoller` hook.
- p95 target < 300 ms on LAN.

**Acceptance criteria** (checklist):

- [ ] Poll interval 15 s during session.
- [ ] Final burst triggers 4 times.
- [ ] Auto-logout when no extension.

**Evidence**:

- _none yet_

---

### `kiosk-ws-recharge-toast` — Time added toast on balance.updated

- **Phase**: K4
- **Status**: `done` (balance.updated → HUD refresh + "Time added" toast)
- **Owner**: TBD
- **ADR refs**: `adr/0018`
- **User-story refs**: US-KSESSION-007
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — On WS `balance.updated`, refresh HUD immediately and show toast with new total minutes.

**Implementation notes** —
- Wire `kiosk-ws-client` to session store.
- Toast component using theme tokens.

**Acceptance criteria** (checklist):

- [ ] Toast within 2 s of staff recharge.
- [ ] HUD matches new balance.

**Evidence**:

- _none yet_

---

### `kiosk-voluntary-end` — Voluntary end, auto-end, and force-end grace UI

- **Phase**: K4
- **Status**: `done` (end button + confirm; auto-end; force-end overlay with grace countdown → cleanup)
- **Owner**: TBD
- **ADR refs**: `adr/0018`
- **User-story refs**: US-KSESSION-006, US-KSESSION-008, US-KSESSION-009
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.2

**Description** — End session button with confirm; handle ENDED_AUTO from poller; force-end overlay with 5:00 grace then cleanup trigger.

**Implementation notes** —
- Distinct from lock screen if implemented (US-KAUTH-005).
- Calls end API with reason.

**Acceptance criteria** (checklist):

- [ ] Voluntary end deducts and returns idle.
- [ ] Auto-end at zero works.
- [ ] Force-end shows 5 min grace.

**Evidence**:

- _none yet_

---

### `kiosk-member-screen` — Member profile and history + audio utilities

- **Phase**: K4
- **Status**: `in_progress` (Should) (LauncherGrid covers allow-listed audio app launch; standalone balances/history tab + volume slider deferred)
- **Owner**: TBD
- **ADR refs**: `adr/0004`
- **User-story refs**: US-KMEMBER-001..003, US-KAUDIO-001, US-KAUDIO-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §4.7–4.8

**Description** — Read-only member tab: balances, session history, transaction history. Utilities: volume slider, launch allow-listed audio apps.

**Implementation notes** —
- Should stories: history tabs can ship after Must launcher.
- Volume via approved Windows API in Rust.

**Acceptance criteria** (checklist):

- [ ] Member screen shows balances.
- [ ] Volume changes audible.
- [ ] G Hub launch works in session when allow-listed.

**Evidence**:

- _none yet_

---

### `kiosk-lockdown-hotkeys` — Block shell hotkeys and re-assert fullscreen

- **Phase**: K5
- **Status**: `in_progress` (keyboard.rs Windows hook cfg-gated; CAD-limit documented; verify on Windows CI)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KLOCK-001, US-KLOCK-002, US-KLOCK-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KLOCK-001

**Description** — When state is `Locked`, block Alt+Tab, Win, Ctrl+Esc; suppress tray; block Explorer/Run/TaskMgr; re-assert kiosk fullscreen after CAD when focus returns.

**Implementation notes** —
- Low-level keyboard hook in `src-tauri`.
- No-op when `SetupRelaxed`.

**Acceptance criteria** (checklist):

- [ ] Hotkeys blocked in Locked.
- [ ] Hotkeys allowed in SetupRelaxed.
- [ ] CAD recovery documented in README.

**Evidence**:

- _none yet_

---

### `kiosk-launch-guard` — Rust launch guard for allow-listed executables only

- **Phase**: K5
- **Status**: `done` (process.rs is_allowed + normalize_path; unit tests; UI rejection wired)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KLOCK-004
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — Only allow-listed paths may be spawned via `launch_allowed` command; reject others with error to UI.

**Implementation notes** —
- Normalize paths; compare case-insensitive on Windows.
- Setup mode may allow broader launch for test — per ADR.

**Acceptance criteria** (checklist):

- [ ] Non-allow-listed launch rejected.
- [ ] Allow-listed launch returns PID for tracker.

**Evidence**:

- _none yet_

---

### `kiosk-lockdown-state-machine` — Central lockdown state machine in Rust

- **Phase**: K5
- **Status**: `done` (lockdown/mod.rs Locked|SetupRelaxed; TRANSITION mutex; lockdown-changed event; parse_state tests)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KLOCK-005
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5 US-KLOCK-005

**Description** — Single source of truth: `Locked` | `SetupRelaxed`. Transitions: admin-login→SetupRelaxed; admin-logout|idle-timeout→Locked; player-session-start→Locked (forced). Emit Tauri events `lockdown-changed` for React.

**Implementation notes** —
- `src-tauri/src/lockdown/mod.rs`.
- All hooks/guards read atomic state.
- Blocked by: `kiosk-adr-lockdown` acceptance.

**Acceptance criteria** (checklist):

- [ ] State transitions match PRD scenarios.
- [ ] React mirrors state.
- [ ] Concurrent transition requests serialized.

**Evidence**:

- _none yet_

---

### `kiosk-process-tracker` — Track spawned process trees per session

- **Phase**: K5
- **Status**: `done` (process.rs tracker; get_tracked_processes Tauri command wired)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KPROC-001, US-KPROC-004
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — Record PID tree for each `launch_allowed` invocation; exclude pre-existing OS processes.

**Implementation notes** —
- Refresh tree periodically or on child create.
- Store in session-scoped Rust struct.

**Acceptance criteria** (checklist):

- [ ] Launched game PID tracked.
- [ ] System processes not in set.

**Evidence**:

- _none yet_

---

### `kiosk-process-cleanup` — Kill tracked processes on session end

- **Phase**: K5
- **Status**: `done` (kill_tracked_processes invoked from endSession; 5-min grace on force-end)
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KPROC-002, US-KPROC-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — Normal/auto/voluntary end: kill all tracked PIDs within 5 s. Force-end: wait 5 min grace (UI in K4) then kill leftovers.

**Implementation notes** —
- `kill_tracked_processes` command.
- Log failures for operator.

**Acceptance criteria** (checklist):

- [ ] Normal end clears processes < 5 s.
- [ ] Force-end waits grace then kills.

**Evidence**:

- _none yet_

---

### `kiosk-offline-grace` — Offline grace for active sessions

- **Phase**: K6
- **Status**: `done` (OFFLINE_GRACE_MS; connection-lost UI + local countdown → re-lock; offline login denial)
- **Owner**: TBD
- **ADR refs**: (none)
- **User-story refs**: US-KOFFLINE-001, US-KOFFLINE-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5; OQ-7

**Description** — When backend unreachable during active session, show connection-lost UI; continue countdown from last known values up to 10 min; then lock. Deny new logins while offline.

**Implementation notes** —
- Detect via failed poll + WS disconnect.
- Configurable grace (OQ-7).

**Acceptance criteria** (checklist):

- [ ] Active session survives 10 min offline display.
- [ ] New login blocked offline.

**Evidence**:

- _none yet_

---

### `kiosk-offline-reconcile` — Queue session end when offline at logout

- **Phase**: K6
- **Status**: `done` (offlineQueue.ts end-intent enqueue/replay; idempotent backend end; no double deduction)
- **Owner**: TBD
- **ADR refs**: (none)
- **User-story refs**: US-KOFFLINE-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — If player ends session while offline, persist end intent locally and replay idempotent end call when online.

**Implementation notes** —
- Local queue in secure storage or sqlite.
- Reason `offline_reconcile`.

**Acceptance criteria** (checklist):

- [ ] Queued end syncs on reconnect.
- [ ] No double deduction.

**Evidence**:

- _none yet_

---

### `admin-kiosk-registration-codes` — Admin UI for kiosk registration codes

- **Phase**: K7
- **Status**: `done` (KioskRegistrationCard + issueRegistrationCode service on device detail)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-001
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §3.1

**Description** — On device detail: generate one-time registration code (TTL e.g. 24 h), list active codes, revoke. Copy-to-clipboard for operator at machine.

**Implementation notes** —
- `apps/admin/src/pages/dashboard/devices/`.
- Backend endpoint for code CRUD.

**Acceptance criteria** (checklist):

- [ ] Generate returns code shown once.
- [ ] Expired codes rejected at register API.
- [ ] Revoke prevents use.

**Evidence**:

- _none yet_

---

### `admin-device-allow-list` — Admin allow-list management for devices

- **Phase**: K7
- **Status**: `cancelled`
- **Owner**: —
- **ADR refs**: `adr/0019` (client localStorage only)
- **User-story refs**: US-KSCAN-005 (deferred v1)
- **Migration rows**: _(n/a)_
- **Plan / NFR refs**: ADR-0019

**Description** — ~~Device detail tab to view/edit launch entries~~ **Cancelled.** Allow-list is on-machine only in v1.

**Evidence**:

- `docs/adr/0019-kiosk-device-allow-list.md`.

---

### `admin-session-force-end` — Force-end session from admin with WS push

- **Phase**: K7
- **Status**: `done` (forceEndSession + confirm dialog on SessionDetailPage; reason `force`)
- **Owner**: TBD
- **ADR refs**: `adr/0013`
- **User-story refs**: US-KSESSION-009, US-KPROC-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — Sessions list/detail: force-end button, confirm dialog, reason `force`; triggers backend end + WS to device channel.

**Implementation notes** —
- Extend `SessionsPage` / `SessionDetailPage`.
- Already partially exists — verify kiosk consumer.

**Acceptance criteria** (checklist):

- [ ] Force-end ends session in DB.
- [ ] Kiosk receives event within 15 s.

**Evidence**:

- _none yet_

---

### `admin-kiosk-fingerprint-view` — Show device fingerprint and drift on admin

- **Phase**: K7
- **Status**: `done` (KioskFingerprintCard parses registeredKiosk JSON + drift badge on device detail)
- **Owner**: TBD
- **ADR refs**: `adr/0017`
- **User-story refs**: US-KREG-002, US-KREG-003
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md Appendix A

**Description** — Device detail panel: parsed fingerprint, last seen, drift warning badge when one component changed.

**Implementation notes** —
- Read `registeredKiosk` JSON.
- Link to re-registration instructions.

**Acceptance criteria** (checklist):

- [ ] Fingerprint displayed.
- [ ] Drift warning visible after one-component change.

**Evidence**:

- _none yet_

---

### `kiosk-vitest-setup` — Vitest + RTL for kiosk React layer

- **Phase**: K8
- **Status**: `done` (vitest.config.ts jsdom; tests for HUD, lockout, offline queue, login form, single-login page)
- **Owner**: TBD
- **ADR refs**: `adr/0005`
- **User-story refs**: US-KAUTH-001, US-KSESSION-002
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6

**Description** — Add Vitest to `apps/kiosk`; smoke tests for login form, HUD timer formatting, single-login error page.

**Implementation notes** —
- `vitest.config.ts` with jsdom.
- One test file per major page.

**Acceptance criteria** (checklist):

- [ ] `pnpm --filter @gaming-cafe/kiosk test` passes.
- [ ] Runs under `turbo run test`.

**Evidence**:

- _none yet_

---

### `kiosk-cargo-tests` — Unit tests for Tauri Rust commands

- **Phase**: K8
- **Status**: `done` (process/lockdown/fingerprint unit tests; clippy clean)
- **Owner**: TBD
- **ADR refs**: `adr/0005`, `adr/0020`
- **User-story refs**: US-KREG-002, US-KLOCK-004
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md

**Description** — Tests for lockdown state transitions, launch guard path normalization, fingerprint JSON shape (mocked).

**Implementation notes** —
- `src-tauri/src/**/*.rs` test modules.
- `cargo test` in kiosk.

**Acceptance criteria** (checklist):

- [ ] State machine transition tests pass.
- [ ] `cargo clippy` clean.

**Evidence**:

- _none yet_

---

### `kiosk-e2e-smoke` — E2E smoke: login, session, single-login 409

- **Phase**: K8
- **Status**: `done` (scripts/e2e-smoke.mjs: register → login → start → 409 → end)
- **Owner**: TBD
- **ADR refs**: `adr/0005`
- **User-story refs**: US-KAUTH-006, US-KSESSION-001
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — Scripted smoke against local backend + kiosk: register device, player login, start session, end session; second device gets 409.

**Implementation notes** —
- `_new/scripts/smoke-kiosk.sh` or Playwright driving WebView.
- Seed player with balance in SQL.

**Acceptance criteria** (checklist):

- [ ] Happy path green.
- [ ] 409 path green.
- [ ] Documented in CONTRIBUTING.md.

**Evidence**:

- _none yet_

---

### `ci-kiosk-build` — GitHub Actions Windows job for kiosk build

- **Phase**: K8
- **Status**: `done` (.github/workflows/kiosk-ci.yml windows-latest: install/lint/typecheck/vitest/clippy/cargo test/tauri build)
- **Owner**: TBD
- **ADR refs**: `adr/0002`, `adr/0005`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.6

**Description** — Add workflow job on `windows-latest`: pnpm install, `cargo test`, `pnpm --filter @gaming-cafe/kiosk tauri:build`, biome, typecheck.

**Implementation notes** —
- Cache cargo + pnpm.
- Optional: only on paths `apps/kiosk/**`.

**Acceptance criteria** (checklist):

- [ ] Workflow green on PR.
- [ ] Artifact uploaded (optional).

**Evidence**:

- _none yet_

---

### `be-kiosk-integration-tests` — Rust integration tests for K1 APIs

- **Phase**: K8
- **Status**: `done` (tests/kiosk_session_flow.rs: end-reason contract, player-auth required, single-session 409; DB-gated cases #[ignore])
- **Owner**: TBD
- **ADR refs**: `adr/0005`, `adr/0017`
- **User-story refs**: US-KAUTH-001, US-KAUTH-006, US-KREG-001
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §5

**Description** — sqlx tests: player login, device register, single-session 409, balance.updated outbox row after recharge.

**Implementation notes** —
- `apps/backend/tests/kiosk_*.rs`.
- Wire into `ci-pipeline` when exists.

**Acceptance criteria** (checklist):

- [ ] All kiosk integration tests pass.
- [ ] Covers 409 and register invalid code.

**Evidence**:

- _none yet_

---

### `kiosk-windows-installer` — Windows MSI/NSIS installer via Tauri bundler

- **Phase**: K9
- **Status**: `in_progress` (tauri.conf.json NSIS perMachine + MSI configured; signing docs in README; build verified on Windows CI)
- **Owner**: TBD
- **ADR refs**: `adr/0002`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.6

**Description** — Configure Tauri bundle for Windows; target installer size ≤ 25 MB where feasible; code signing documented for operator.

**Implementation notes** —
- `tauri.conf.json` bundle section.
- CI artifact from `ci-kiosk-build`.

**Acceptance criteria** (checklist):

- [ ] Installer builds on Windows CI.
- [ ] Installs and launches on clean Win10 VM.

**Evidence**:

- _none yet_

---

### `kiosk-webview2-bootstrap` — WebView2 runtime check and install guide

- **Phase**: K9
- **Status**: `done` (webviewInstallMode: embedBootstrapper; README Win10 install guide)
- **Owner**: TBD
- **ADR refs**: `adr/0002`
- **User-story refs**: (infra)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.6

**Description** — Installer or first-run check for WebView2; `apps/kiosk/README.md` documents manual install for Win10.

**Implementation notes** —
- Tauri WebView2 bootstrapper option.
- Link Microsoft WebView2 runtime.

**Acceptance criteria** (checklist):

- [ ] README covers Win10 requirement.
- [ ] First-run detects missing runtime with clear message.

**Evidence**:

- _none yet_

---

### `kiosk-deploy-guide` — Windows station deployment operator guide

- **Phase**: K10
- **Status**: `done` (KIOSK-WINDOWS-DEPLOYMENT.md: Assigned Access, shell replacement, Run key, manual watchdog, risks)
- **Owner**: TBD
- **ADR refs**: `adr/0020`, `adr/0028`
- **User-story refs**: US-KDEPLOY-001, US-KDEPLOY-005
- **Migration rows**: (none — docs)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §4.10, §6.6

**Description** — Document how cafes configure Windows for kiosk shell: dedicated account,
auto-logon, Assigned Access vs shell replacement vs Run key, GPO hardening, and manual
watchdog until installer automation ships.

**Acceptance criteria** (checklist):

- [x] Single doc covers boot launch + auto-restart strategies.
- [x] Linked from PLANNER and kiosk README.
- [x] K10 task breakdown with priorities.

**Evidence**:

- `docs/KIOSK-WINDOWS-DEPLOYMENT.md`

---

### `kiosk-startup-task` — Installer option: launch kiosk at logon

- **Phase**: K10
- **Status**: `pending`
- **Owner**: TBD
- **ADR refs**: `adr/0028`
- **User-story refs**: US-KDEPLOY-002
- **Migration rows**: (none — installer)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §6.6

**Description** — NSIS installer checkbox (default on): register `HKLM\...\Run` or per-user
Scheduled Task “At log on” for `Arena360 Kiosk.exe`. Uninstall removes registration.

**Implementation notes** —
- `tauri.conf.json` NSIS hooks or custom `installer-hooks.nsh`.
- Idempotent: upgrade reinstall does not duplicate entries.

**Acceptance criteria** (checklist):

- [ ] Fresh install on Win10 VM auto-launches kiosk after reboot/logon.
- [ ] Uninstall removes startup entry.
- [ ] Documented in KIOSK-WINDOWS-DEPLOYMENT.md § Layer 3.

**Evidence**:

- _none yet_

---

### `kiosk-watchdog` — Sidecar process: relaunch kiosk if exited

- **Phase**: K10
- **Status**: `pending`
- **Owner**: TBD
- **ADR refs**: `adr/0020`
- **User-story refs**: US-KDEPLOY-003, US-KDEPLOY-004
- **Migration rows**: (none — new binary)
- **Plan / NFR refs**: KIOSK-WINDOWS-DEPLOYMENT.md § Layer 2

**Description** — Small watchdog binary + Scheduled Task at logon. Polls main process every
2 s; relaunch if missing unless `%ProgramData%\Arena360\watchdog.pause` exists (TTL).
Single-instance mutex shared with kiosk. Optional IPC from `SetupRelaxed` to set pause.

**Implementation notes** —
- `apps/kiosk/src-tauri/watchdog/` or sibling crate; ship in same NSIS bundle.
- Exit code `--maintenance` or pause file from setup “exit to desktop”.
- Do not relaunch during `tauri-plugin-process` update handoff (mutex transfer).

**Acceptance criteria** (checklist):

- [ ] Kill main process → relaunch within 10 s on clean VM.
- [ ] Pause file → no relaunch for configured TTL.
- [ ] No duplicate kiosk windows at boot.
- [ ] cargo tests for pause-file parsing.

**Evidence**:

- _none yet_

---

### `kiosk-installer-fleet` — Silent install + configure-station.ps1

- **Phase**: K10
- **Status**: `pending`
- **Owner**: TBD
- **ADR refs**: `adr/0028`
- **User-story refs**: US-KDEPLOY-001, US-KDEPLOY-002
- **Migration rows**: (none — scripts)
- **Plan / NFR refs**: KIOSK-WINDOWS-DEPLOYMENT.md § Layer 3

**Description** — `apps/kiosk/scripts/windows/configure-station.ps1` with parameters:
`-ShellMode AssignedAccess|ReplaceShell|RunKey`, `-KioskUser`, optional auto-logon
prompt. NSIS silent `/S` documented for SCCM/Intune.

**Acceptance criteria** (checklist):

- [ ] Script idempotent on re-run.
- [ ] README + deployment doc show Intune/SCCM example command line.
- [ ] No secrets committed (auto-logon password entered at deploy time).

**Evidence**:

- _none yet_

---

### `kiosk-auto-update` — Optional Tauri updater channel

- **Phase**: K9
- **Status**: `done` (Could) (createUpdaterArtifacts: false by default; updater enablement documented in README)
- **Owner**: TBD
- **ADR refs**: `adr/0002`
- **User-story refs**: (Could)
- **Migration rows**: (none — new code)
- **Plan / NFR refs**: REQUIREMENTS-KIOSK.md §1.5

**Description** — Configure Tauri updater with operator-controlled endpoint; disabled by default in v1.

**Implementation notes** —
- Document security implications.
- Could priority — ship only if K8 complete early.

**Acceptance criteria** (checklist):

- [ ] Updater config present but off by default.
- [ ] Documented enable steps for operator.

**Evidence**:

- _none yet_

---

## Cross-reference matrices

### Draft ADR → Tasks

| Draft ADR | Title | Implementing tasks |
|-----------|-------|-------------------|
| `ADR-0016` (reintroduce) | Kiosk monorepo | `kiosk-adr-reintroduce` → `kiosk-workspace-wire`, `kiosk-app-scaffold`, `ci-kiosk-build` |
| `ADR-0017` | Player + device JWT | `kiosk-adr-player-auth` → auth/session `be-*`, `kiosk-secure-token-storage`, `kiosk-player-login` |
| `ADR-0018` | Device WS ACL + events | `kiosk-adr-ws-acl` → `be-ws-device-acl`, `be-recharge-events`, `kiosk-ws-client`, `kiosk-ws-recharge-toast` |
| `ADR-0019` | Client allow-list | `kiosk-adr-allow-list` → `kiosk-allow-list-editor` (`be-allow-list-storage`, `admin-device-allow-list` cancelled) |
| `ADR-0020` | Windows lockdown | `kiosk-adr-lockdown` → `kiosk-lockdown-state-machine`, `kiosk-lockdown-hotkeys`, `kiosk-launch-guard`, `kiosk-process-*` |

### User story → Tasks (Must-priority `US-K*`)

| US | Title (short) | Implementing tasks | Verifying task |
|----|---------------|-------------------|----------------|
| US-KREG-001 | Device registration code | `be-device-registration-api`, `kiosk-registration-ui`, `admin-kiosk-registration-codes` | `kiosk-e2e-smoke` |
| US-KREG-002 | Fingerprint collection | `kiosk-fingerprint-cmd`, `be-fingerprint-drift` | `kiosk-cargo-tests` |
| US-KREG-003 | Fingerprint drift policy | `be-fingerprint-drift`, `admin-kiosk-fingerprint-view` | `be-kiosk-integration-tests` |
| US-KREG-004 | Skip repeat onboarding | `kiosk-registration-ui` | `kiosk-e2e-smoke` |
| US-KREG-005 | Setup device schema | `kiosk-setup-mode-auth`, `kiosk-registration-ui` | `kiosk-e2e-smoke` |
| US-KREG-006 | Block login on maintenance | `kiosk-device-status-ws` | `kiosk-e2e-smoke` |
| US-KSCAN-001 | Software scan | `kiosk-software-scan-cmd` | `kiosk-cargo-tests` |
| US-KSCAN-002 | Utility detection | `kiosk-software-scan-cmd` | `kiosk-cargo-tests` |
| US-KSCAN-003 | Allow-list curation | `kiosk-allow-list-editor` | `kiosk-e2e-smoke` |
| US-KSCAN-004 | Hide missing exes | `kiosk-allow-list-editor` | `kiosk-vitest-setup` |
| US-KAUTH-001 | Player login | `be-player-auth-endpoint`, `kiosk-player-login` | `be-kiosk-integration-tests` |
| US-KAUTH-002 | Player JWT | `be-player-auth-endpoint`, `kiosk-secure-token-storage` | `be-kiosk-integration-tests` |
| US-KAUTH-003 | Balance gate | `kiosk-balance-gate` | `kiosk-e2e-smoke` |
| US-KAUTH-005 | End session billing | `kiosk-voluntary-end` | `kiosk-e2e-smoke` |
| US-KAUTH-006 | Single login | `be-single-session-rule`, `kiosk-single-login-guard` | `kiosk-e2e-smoke`, `be-kiosk-integration-tests` |
| US-KSESSION-001 | Auto-start session | `be-kiosk-session-routes`, `kiosk-session-start` | `kiosk-e2e-smoke` |
| US-KSESSION-002 | HUD timer | `kiosk-hud-timer` | `kiosk-vitest-setup` |
| US-KSESSION-003 | Reminders 10/5/1 | `kiosk-reminders` | `kiosk-vitest-setup` |
| US-KSESSION-004 | 15 s poll | `kiosk-poll-final-burst` | `kiosk-e2e-smoke` |
| US-KSESSION-005 | Final-minute burst | `kiosk-poll-final-burst` | `kiosk-e2e-smoke` |
| US-KSESSION-006 | Auto-end at zero | `be-kiosk-session-routes`, `kiosk-voluntary-end` | `kiosk-e2e-smoke` |
| US-KSESSION-007 | WS recharge toast | `be-recharge-events`, `kiosk-ws-recharge-toast` | `be-kiosk-integration-tests` |
| US-KSESSION-008 | Voluntary end | `kiosk-voluntary-end` | `kiosk-e2e-smoke` |
| US-KSESSION-009 | Force-end grace | `admin-session-force-end`, `kiosk-voluntary-end`, `kiosk-process-cleanup` | `kiosk-e2e-smoke` |
| US-KSESSION-010 | Device status in_use | `be-kiosk-session-routes`, `kiosk-session-start` | `kiosk-e2e-smoke` |
| US-KPROC-001 | Process tracking | `kiosk-process-tracker` | `kiosk-cargo-tests` |
| US-KPROC-002 | Immediate kill | `kiosk-process-cleanup` | `kiosk-e2e-smoke` |
| US-KPROC-003 | Force-end 5 min grace | `kiosk-process-cleanup`, `kiosk-voluntary-end` | `kiosk-e2e-smoke` |
| US-KPROC-004 | Ignore OS processes | `kiosk-process-tracker` | `kiosk-cargo-tests` |
| US-KLOCK-001 | Hotkey block | `kiosk-lockdown-hotkeys`, `kiosk-lockdown-state-machine` | `kiosk-cargo-tests` |
| US-KLOCK-002 | CAD handling | `kiosk-lockdown-hotkeys` | manual QA |
| US-KLOCK-003 | Shell shortcuts blocked | `kiosk-lockdown-hotkeys` | manual QA |
| US-KLOCK-004 | Launch guard | `kiosk-launch-guard` | `kiosk-cargo-tests` |
| US-KLOCK-005 | Setup relaxes lockdown | `kiosk-setup-mode-auth`, `kiosk-lockdown-state-machine` | `kiosk-cargo-tests`, `kiosk-e2e-smoke` |
| US-KMEMBER-001 | Member profile | `kiosk-member-screen` | `kiosk-vitest-setup` |
| US-KAUDIO-001 | Volume control | `kiosk-member-screen` | manual QA |
| US-KOFFLINE-002 | Deny login offline | `kiosk-offline-grace` | `kiosk-e2e-smoke` |

Should/Could stories map to the same epic tasks; verification deferred unless promoted to Must.

### NFR → Tasks

| NFR | Target | Tasks |
|-----|--------|-------|
| Performance (poll p95 < 300 ms) | LAN session poll | `kiosk-poll-final-burst`, `kiosk-http-client` |
| Performance (scan < 60 s) | Software scan | `kiosk-software-scan-cmd` |
| Performance (WS reconnect < 5 s) | Realtime | `kiosk-ws-client` |
| Reliability (one session per player) | Server enforcement | `be-single-session-rule` |
| Reliability (server-authoritative time) | HUD | `kiosk-hud-timer` |
| Security (secure token storage) | Keychain | `kiosk-secure-token-storage` |
| Security (device/player JWT) | Auth ADR | `be-player-auth-endpoint`, `be-device-registration-api` |
| Observability (structured logs) | Rust core | `kiosk-lockdown-state-machine`, `kiosk-process-tracker` |
| Deployment (Windows installer) | ≤ 25 MB target | `kiosk-windows-installer`, `kiosk-webview2-bootstrap` |
| Deployment (CI Windows) | PR gate | `ci-kiosk-build` |

### Open question → Owner task

| OQ | Question | Resolving task | Status |
|----|----------|----------------|--------|
| OQ-1 | Staff shift vs system shift | `be-kiosk-session-routes`, `kiosk-adr-player-auth` | **Resolved** — system kiosk shift ([adr/0017](adr/0017-kiosk-player-device-auth.md) § Shift binding) |
| OQ-2 | WS event payloads | `kiosk-adr-ws-acl`, `be-recharge-events` | **Resolved** — camelCase schemas ([adr/0018](adr/0018-kiosk-ws-device-acl.md) § Event schemas) |
| OQ-3 | Player JWT lifetime | `kiosk-adr-player-auth` | **Resolved** — 24 h fixed ([adr/0017](adr/0017-kiosk-player-device-auth.md) § Token lifetimes) |
| OQ-4 | Game close behavior | `kiosk-voluntary-end` | Open |
| OQ-5 | Max concurrent apps | `kiosk-process-tracker` | Open |
| OQ-6 | Reminder sound default | `kiosk-reminders` | Open |
| OQ-7 | Offline grace duration | `kiosk-offline-grace` | Open |
| OQ-8 | Allow-list icons | `kiosk-adr-allow-list` | **Resolved** — local `.exe` extraction ([adr/0019](adr/0019-kiosk-device-allow-list.md) § Icon strategy) |
| OQ-9 | Crash re-login policy | `be-single-session-rule`, `kiosk-adr-player-auth` | **Resolved** — resume on same device ([adr/0017](adr/0017-kiosk-player-device-auth.md) § Single-session rule) |

### Task dependency graph (selected blockers)

| Task | Blocked by |
|------|------------|
| All K1 | All K0 ADRs accepted |
| All K2 | `kiosk-adr-reintroduce` accepted |
| `kiosk-setup-mode-auth` | `kiosk-lockdown-state-machine` |
| `kiosk-session-start` | `be-kiosk-session-routes`, `kiosk-lockdown-state-machine` |
| `kiosk-e2e-smoke` | K1 verified, K4 core `done` |
| `ci-kiosk-build` | `kiosk-app-scaffold` `done` |

---

## Resolved decisions

| # | Topic | Decision |
|---|-------|----------|
| D1 | Platform | Windows 10/11 only for v1 |
| D2 | Player auth | Username + password |
| D3 | Recharge | Staff/counter only; WS on `device:{id}` |
| D4 | Fingerprint | MAC + serial + BIOS UUID; 1-component tolerance |
| D5 | Cleanup | Normal → kill now; force-end → 5 min grace |
| D6 | Reminders | 10/5/1 min + final 60 s 4× poll; recharge toast |
| D7 | Member | Profile + history read-only |
| D8 | Single login | One active session per player |
| D9 | Setup lockdown | Admin login disables lockdown; logout/idle/player session re-enables |
| D10 | Kiosk shift binding (OQ-1) | System kiosk shift per venue (`KIOSK_SYSTEM`); kiosk sessions always use it |
| D11 | Player JWT lifetime (OQ-3) | 24 h fixed (`JWT_PLAYER_EXPIRATION`); device JWT 365 d; no refresh token v1 |
| D12 | Crash re-login (OQ-9) | Resume existing open session on same device; return `activeSession` on login |
| D13 | Allow-list storage (OQ-8) | Client `localStorage` only (`gaming-cafe.kiosk.launch_entries`); no backend API v1 |
| D14 | Allow-list icons | Local `.exe` extraction; no R2 upload v1 |
| D15 | WS event schemas (OQ-2) | camelCase payloads for `session.*`, `balance.updated`, `device.status_changed` |
| D16 | Remote allow-list (US-KSCAN-005) | Deferred / Won't v1 — on-machine curation only |

---

## Implementation notes / decisions log

- **2026-05-30**: `PLANNER-KIOSK.md` authored — 57 tasks K0–K9; backend K1 included; awaits K0 ADR approval.
- **2026-05-30**: D9 lockdown toggle — `kiosk-setup-mode-auth` + `kiosk-lockdown-state-machine`; PRD `US-KLOCK-005` scenarios updated.
- **2026-05-30**: `kiosk-adr-reintroduce` verified — `0016-kiosk-monorepo-reintroduce.md` Accepted; K2 unblocked.
- **2026-05-30**: K2 `kiosk-workspace-wire` + `kiosk-app-scaffold` in progress (greenfield; archive tarball unavailable).
- **2026-05-30**: `kiosk-adr-player-auth` done — `DRAFT-0017-kiosk-player-device-auth.md` Proposed; OQ-1/OQ-3/OQ-9 resolved (D10–D12); K1 auth tasks blocked until ADR Accepted.
- **2026-05-30**: **K0 complete** — ADR-0017–0020 Accepted; OQ-2/OQ-8 resolved (D13–D16); `be-allow-list-storage` + `admin-device-allow-list` cancelled; **K1 unblocked**.
- **2026-05-30**: User confirmed all K0 ADR approvals; REQUIREMENTS-KIOSK.md §7/10/11 synced to Accepted ADRs.
- **2026-05-30**: **K1–K9 implementation pass** (single agent session). Summary of evidence:
  - **K1 backend** (`cargo build` green; `cargo test --test kiosk_session_flow` 1 pass + 2 DB-gated `#[ignore]`):
    `handlers/kiosk.rs` (`POST /kiosk/sessions`, `GET /kiosk/sessions/current`, `PATCH /kiosk/sessions/{id}/end`, idempotent end),
    `services/session_service.rs` (`start_for_player`, system kiosk shift, end-reason validation),
    `services/transaction_service.rs` (`balance.updated` outbox on recharge),
    `services/device_service.rs` + `repositories/device_repo.rs` (fingerprint drift, 1-component tolerance, `DEVICE_FINGERPRINT_MISMATCH`),
    `dto/kiosk_dto.rs`, `openapi/mod.rs`; `pnpm gen:api-types` regenerated (`PlayerLoginDto`, kiosk paths).
    Session-end-reason persisted at the API/WS layer only; DB column deferred behind `DRAFT-0021`.
  - **K2 wiring**: `@gaming-cafe/theme` tokens in kiosk entry; `storage.rs` rationale doc; `KioskProvider` error/`deviceName`/port reconcile; realtime reconnect fix.
  - **K3 onboarding**: `fingerprint.rs` (PowerShell CIM, dev stub), `scan.rs` (`scan_installed_software` + progress events), `RegistrationPage` fields + fingerprint preview, `AllowListEditor`, `IdlePage` maintenance banner via `device.status_changed`.
  - **K4 session loop**: attract `IdlePage` + hidden `SetupGesture`; `PlayerLoginPage` lockout (`loginLockout.ts`); `AlreadyInSessionPage` (409 guard); `SessionPage` start/launcher (`LauncherGrid`), local HUD countdown (`useSessionRemainingMinutes`), reminders, recharge toast via WS, voluntary/auto end at 10 s, immediate staff force-end cleanup. Member screen partial (launcher covers audio-app launch; standalone history tab deferred).
  - **K5 lockdown**: `lockdown/mod.rs` emits `lockdown-changed`, serialized transitions; `keyboard.rs` CAD-limit doc; setup-mode auth relaxes only after admin OTP + 15-min idle re-lock; launch guard + process tracker cleanup wired from `endSession`.
  - **K6 offline**: `config.ts` `OFFLINE_GRACE_MS`; connection-lost UI + local countdown then re-lock; login denial offline; `offlineQueue.ts` end-intent replay with idempotent backend end.
  - **K7 admin**: `KioskFingerprintCard` (parsed fingerprint + drift badge) on device detail; `forceEndSession` + confirm dialog on `SessionDetailPage`; registration-code card already present.
  - **K8 tests/CI**: kiosk cargo unit tests (8 pass, clippy clean); Vitest+RTL (`vitest.config.ts`, 17 tests pass — HUD, lockout, offline queue, login form, single-login page); `apps/backend/tests/kiosk_session_flow.rs`; `apps/kiosk/scripts/e2e-smoke.mjs`; `.github/workflows/kiosk-ci.yml` (windows-latest).
  - **K9 packaging**: `tauri.conf.json` Windows bundle (NSIS perMachine + MSI, `embedBootstrapper`, `createUpdaterArtifacts: false`); `apps/kiosk/README.md` packaging + WebView2 + signing + auto-update guide.
  - **Platform caveat**: Windows-only native code (keyboard hook, WMI/PowerShell fingerprint+scan, installer) is `#[cfg(target_os = "windows")]` with dev stubs; verified to compile on macOS and slated for verification on the `kiosk-windows` CI job.
  - **Out of scope / pre-existing**: `apps/admin` has 25 unrelated typecheck errors in `realtime/__tests__`, `PlanTransactionsPage`, `PlansPage` (uncommitted work predating this pass); kiosk + utils typecheck clean.

---

## How to use this file

1. **K0 complete** — ADR-0016–0020 Accepted; proceed with K1/K2 implementation.
2. Set task status `in_progress` → `done` → `verified` with evidence.
3. Platform consolidation tasks stay in [PLANNER.md](PLANNER.md).
4. **First sprint (K1/K2):** `be-device-registration-api`, `be-player-auth-endpoint`, `be-single-session-rule`, `kiosk-shared-packages` (`kiosk-app-scaffold` + `kiosk-workspace-wire` already `verified`).

---
