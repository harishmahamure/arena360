# ADR-0005: Testing strategy and coverage ramp

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

Across the three source repos, test coverage is effectively zero:

- `arena360-backend` has a Jest config and one or two stub specs that
  do not exercise real code paths.
- `game-zone-management-fe` has Vitest configured in some libraries
  but no meaningful tests.
- `agent-kiosk` has no JS tests; `src-tauri/` has only auto-generated
  Rust integration test scaffolding.

Demanding a high coverage gate on day one of the new monorepo would
either freeze the consolidation (we'd be writing tests instead of
moving files) or push everyone to write low-value tests just to clear
a percentage. Neither helps.

We also have three runtimes (Node for the backend, browser/Vite for
admin and kiosk, Rust for kiosk-tauri) that each have a different
preferred test runner. Forcing one tool across all three would slow
us down for no real benefit.

## Decision

**Test runner per workspace** (we use the native fit):

| Workspace | Runner | Reasoning |
|---|---|---|
| `apps/backend` | **Jest** + `@nestjs/testing` | NestJS's testing module is built around Jest; ecosystem is mature. |
| `apps/admin` | **Vitest** + React Testing Library (RTL) | Same Vite config as dev; fast HMR for tests; first-class TS. |
| `apps/kiosk` (React side) | **Vitest** + RTL | Same reasoning as admin. |
| `apps/kiosk` (Rust crate `src-tauri/`) | **`cargo test`** | The standard Rust test runner. |
| `packages/*` (TS libraries) | **Vitest** | Lightweight, fast; matches the FE story. |

**Initial bar: one smoke test per workspace.** During Phase 7 of the
consolidation, every app and package ships exactly one passing test
that exercises the happy path of one real code path. Examples:

- `apps/backend`: a Jest test for `AuthService.login()` issuing a
  valid JWT (see `US-AUTH-001` + `US-AUTH-002`).
- `apps/admin`: a Vitest + RTL test for the root route renders the
  login screen and the brand header.
- `apps/kiosk` (React): a Vitest test for the session-poll hook's
  state-transition logic (mocked HTTP).
- `apps/kiosk` (Rust): a `cargo test` for `scan_installed_software`'s
  happy path with a mocked filesystem.
- `packages/ui`: a Vitest + RTL test for `<DataGrid />` rendering
  with one row.
- `packages/utils`: a Vitest test for `unwrapEnvelope()` happy and
  error paths.
- `packages/theme`: a Vitest test asserting `tokens.css` contains
  every documented custom property.

**Coverage ramp**:

| Date | Required line coverage gate (per workspace) |
|---|---|
| Consolidation cutover (today) | **0%** (gate present but threshold = 0; CI still produces a coverage report) |
| Cutover + 3 months | **5%** |
| Cutover + 6 months | **10%** |
| Cutover + 9 months | **15%** |
| Cutover + 12 months | **20%** (interim) — target **60%** by the start of year two |

Each quarter the gate ratchets up by 5 percentage points. Lowering the
gate ever requires an ADR superseding this one.

**Coverage is per workspace, not aggregated**, so a low-coverage
package can't drag down a well-covered one (and vice versa).

**What we count**:

- TS workspaces: line coverage from `@vitest/coverage-v8` (or Jest's
  built-in V8 reporter).
- Rust crate: line coverage via `cargo-llvm-cov`, reported alongside
  the JS coverage in CI.

**What we don't gate (today)**: branch coverage, function coverage,
end-to-end tests. Branch/function coverage become advisory once line
coverage clears 20%; e2e tests are deferred until a real flow needs
them (see Risks below).

## Consequences

### Positive

- **No theatre.** Day-one tests are real (one smoke test per workspace),
  not "100% coverage on a 5-line file" vanity.
- **Steady, predictable ratchet.** Engineers can plan: in three months
  we need 5% coverage on touched workspaces, so we add tests as we
  touch code.
- **Right tool per workspace.** No fighting Jest in a Vite app or
  Vitest in a Nest app.
- **Coverage gate exists from day one** even at 0% — the wiring is
  done, the report is published, only the threshold ratchets.
- **Rust included.** `cargo test` is non-negotiable for kiosk-critical
  native code; this ADR makes it first-class in CI.

### Negative

- **Two test runners.** Onboarding a contributor who touches both
  backend and admin requires learning both Jest and Vitest. Mitigated
  by their API similarity (`describe`, `it`, `expect`).
- **Coverage numbers are not comparable across runners.** V8 line
  coverage from Vitest and Jest can disagree on edge cases (e.g.
  inlined conditionals). Acceptable because gates are per-workspace.
- **The 60%-in-12-months target is a stretch** for some packages (the
  generated `packages/api-types` has nothing to test). We exclude
  generated files from the denominator.

### Risks

- **Risk**: coverage chasing produces low-value tests.
  **Mitigation**: the ratchet is gentle (+5%/quarter); reviewers
  reject "assert that the constant equals itself" tests.
- **Risk**: no e2e tests means regressions in the kiosk's
  full-system flow are caught only manually.
  **Mitigation**: this is acknowledged. A follow-up ADR will introduce
  an e2e harness (likely Playwright for admin + a manual kiosk
  smoke checklist) once the consolidation is stable. Cross-app e2e
  is explicitly out of scope for the initial ramp.
- **Risk**: flaky tests erode the ratchet's value.
  **Mitigation**: any test marked `.skip` requires a tracking issue
  link in a comment; `.skip` without an issue link fails the lint
  rule (configured in `@gaming-cafe/biome-config`).

## Alternatives considered

### One runner across all TS workspaces (Vitest everywhere)

- Pros: single mental model, one configuration to maintain.
- Cons: NestJS testing utilities assume Jest globals; the Nest
  community standard is Jest; pulling Vitest into the backend means
  fighting both the framework and the docs.
- **Why rejected**: per-workspace native fit is worth the small
  learning cost.

### High initial coverage gate (e.g. 60% from day one)

- Pros: forces quality up-front.
- Cons: blocks the consolidation; rewards quantity-over-quality
  tests; the existing codebase has near-zero tests so the gate would
  be unreachable without a multi-week pause.
- **Why rejected**: optimises for the wrong thing at the wrong time.

### No coverage gate, ever (informational only)

- Pros: zero friction.
- Cons: coverage tends toward zero without a forcing function; this
  is what the source repos already proved.
- **Why rejected**: empirically does not work.

### Mutation testing (Stryker) instead of line coverage

- Pros: catches assertion-free tests; better signal.
- Cons: slow; tooling for NestJS + Vitest still maturing; we don't
  have the baseline to even run it usefully.
- **Why rejected**: revisit once we are routinely above 30% line
  coverage.

## Implementation notes

- Each workspace's `package.json` includes a `test` script and a
  `test:coverage` script. Turborepo's `test` pipeline runs whichever
  CI configures (we run `test:coverage` in CI and `test` locally for
  speed).
- Coverage thresholds live in each workspace's `vitest.config.ts` /
  `jest.config.ts` / `Cargo.toml` so they can be adjusted independently
  when the ratchet ticks.
- The ratchet itself is a calendar task on the platform team's
  quarterly review.
- The smoke-test rollout is tracked as `be-first-test`,
  `fe-first-test`, and `kiosk-first-test` in the consolidation plan.

## References

- ADR-0001: Turborepo + pnpm (where `test` is plumbed).
- ADR-0006: Biome tooling (`.skip` lint rule lives there).
- Consolidation plan, Phase 7 (`be-first-test`, `fe-first-test`,
  `kiosk-first-test`).
