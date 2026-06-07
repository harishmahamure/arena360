# DRAFT ADR-0034: Admin app shell v2 (Sidebar, AppBar, page chrome)

**Status**: Proposed
**Date**: 2026-06-07
**Deciders**: Platform team (pending user approval)

**Extends**: [ADR-0007](0007-design-tokens-shared.md) (shared design tokens)
**Supersedes**: _(none)_

## Context

The admin SPA (`apps/admin`) uses a shared shell from `packages/ui`:

- `DashboardLayout` — flex wrapper around sidebar + app bar + outlet
- `Sidebar` — navy drawer with permission-filtered `adminNavItems`
- `AppBar` — fixed top bar with profile menu and optional POS/plan shortcuts

Recent UX gap work fixed functional issues (shift guards, TOTP dialogs, staff
CTAs, permission toasts). The shell still has structural and visual debt:

| Issue | Impact |
|-------|--------|
| Legacy theme (`lib/theme.ts`) not fully token-driven | Brand colours hardcoded in Sidebar active states |
| Collapsed sidebar hides child nav with no flyout | Staff lose POS sub-links when sidebar auto-collapses |
| AppBar has no contextual page title | Operators lose place context on deep routes |
| No shared `PageShell` | ~52 pages duplicate padding (`px: 4, py: 2` vs `py: 3–4`) |
| E-commerce `defaultNavItems` still in Sidebar | Dead code; confusing for contributors |
| Login (dark glass) vs dashboard (flat light) | Weak brand continuity |

[PLAN-ADMIN-UI-MODERNIZATION.md](../PLAN-ADMIN-UI-MODERNIZATION.md) and
[TASKS-ADMIN-UI-MODERNIZATION.md](../TASKS-ADMIN-UI-MODERNIZATION.md) define
the full modernization programme. **This ADR scopes only the app shell and
page chrome primitives** (Milestones M1–M2 + `PageShell`), not list/form page
migrations or bespoke screens (POS, session detail).

Per [`.cursor/rules/20-adr-discipline.mdc`](../../.cursor/rules/20-adr-discipline.mdc),
changes to shared layout components in `packages/ui` that alter the admin
navigation contract are architectural and require approval before implementation.

## Decision

Upgrade the admin shell in **`packages/ui`** and wire it from **`apps/admin`**
using existing MUI v7 and ADR-0007 tokens. **No new UI library.**

### 1. Theme consolidation (prerequisite)

- Merge production overrides from `packages/theme/src/lib/theme.ts` into
  `packages/theme/src/mui-theme.ts` (built from `tokens.ts`).
- Switch `packages/providers` to export `muiTheme` as the single admin theme.
- Deprecate `lib/theme.ts` with a pointer comment; remove after M1 verification.

### 2. `PageShell` primitive

Add `PageShell` to `@gaming-cafe/ui`:

```ts
interface PageShellProps {
  children: React.ReactNode;
  maxWidth?: number | 'sm' | 'md' | 'lg';
  header?: React.ReactNode;   // PageHeader slot
  toolbar?: React.ReactNode;  // filter chips, actions row
  footer?: React.ReactNode;   // pagination
}
```

Default padding: `px: { xs: 2, md: 4 }`, `py: { xs: 2, md: 3 }`.

Admin `PageHeader` (breadcrumbs, back, title) may move into `packages/ui` during
M1 or remain in `apps/admin` as a thin wrapper — implementation detail.

### 3. Sidebar v2

| Change | Detail |
|--------|--------|
| Active state | Left accent bar + `primary.main` tint from theme tokens (no hardcoded `rgba(255,105,0,…)`) |
| Section labels | Optional `section?: string` on nav items; render `Divider` + caption between groups |
| Collapsed flyout | When drawer width = collapsed, child routes open in `Popover`/`Menu` anchored to parent icon |
| Cleanup | Remove unused `defaultNavItems` e-commerce placeholder |

Nav **paths unchanged** — label-only grouping in `apps/admin/src/constants/navItems.tsx`.

### 4. AppBar v2

| Change | Detail |
|--------|--------|
| Contextual title | Route → title map passed from admin layout (e.g. `/sessions` → "Sessions") |
| Shift badge | Staff only: chip showing active shift duration or "No shift" with link to dashboard |
| Quick actions | Keep POS/plan icons; continue gating by permission + active shift (existing behaviour) |
| Dead controls | Search and theme toggle remain **off** until implemented |

### 5. Nav grouping (admin config only)

Group `adminNavItems` into labelled sections without changing routes:

1. **Operations** — Sessions, POS sales, Plan sales, Running tab
2. **People & devices** — Players, Devices
3. **Catalog** — Products, Plans, Games
4. **Finance** — Cash registers, Cash deposits, Expenses, Shifts
5. **System** — Settings, Inventory (admin-only)

Permission filtering (`filterNavItemsByPermission`) unchanged.

### 6. Out of scope (separate tasks / future ADR)

- `ListPage` / `FormPage` / `DetailPage` composites (TASKS UI-008–011)
- Dark-mode dashboard
- MUI X DataGrid
- Kiosk shell changes
- Backend or API changes

## Consequences

### Positive

- **Single theme path** aligns runtime with ADR-0007 documentation.
- **Collapsed sidebar usable** during rush-hour POS flows.
- **Consistent page chrome** reduces per-page `sx` duplication.
- **Incremental adoption** — pilot 5 pages first; no big-bang rewrite.

### Negative

- **Two-package coordination** — every shell change requires `packages/ui` build
  before admin typecheck passes.
- **Sidebar API extension** — `NavItem` may gain `section?: string`; admin
  `AdminNavItem` type must stay in sync.

### Risks

| Risk | Mitigation |
|------|------------|
| Theme merge breaks login dark theme | Gate on login + 3 list pages before wide rollout |
| Flyout conflicts with mobile drawer | Use `Popover` only when `collapsed && !isMobile` |
| Shift badge stale | Reuse existing `activeShift` react-query key |

## Alternatives considered

### A. Rebuild shell in `apps/admin` only (not `packages/ui`)

- **Pros**: Faster iteration, no shared-package publish cycle.
- **Cons**: Duplicates layout if kiosk admin patterns emerge; violates monorepo
  intent for shared primitives.
- **Rejected**: Shell is already shared; extend in place.

### B. Adopt a third-party admin template (e.g. Materio, Minimal)

- **Pros**: Polished out of the box.
- **Cons**: New dependency, licensing, fight with existing `FormBuilder` /
  `DataGrid`; conflicts with ADR-0007 token ownership.
- **Rejected**.

### C. Full dark dashboard to match login

- **Pros**: Strong brand continuity.
- **Cons**: ~50 pages to restyle; poor readability for dense tables during
  owner reporting.
- **Deferred** to backlog (PLAN Phase 5 optional).

## Implementation notes

### Pilot pages (gate each milestone)

1. `SessionsPage`
2. `StaffDashboardView`
3. `PlanTransactionsPage`
4. `SessionDetailPage`
5. `LoginPage`

### Task mapping

| ADR scope | TASKS IDs |
|-----------|-----------|
| Theme | UI-001 |
| PageShell | UI-002 |
| Toasts | UI-003 |
| Sidebar v2 | UI-005 |
| AppBar v2 | UI-006 |
| Nav grouping | UI-007 |

### Verification

- `pnpm --filter @gaming-cafe/ui build`
- `pnpm --filter @gaming-cafe/admin typecheck`
- Manual: collapsed sidebar → POS child routes reachable
- Manual: staff AppBar shows shift state
- Responsive: 375px / 768px / 1280px on pilot pages

## References

- [ADR-0007: Shared design tokens](0007-design-tokens-shared.md)
- [PLAN-ADMIN-UI-MODERNIZATION.md](../PLAN-ADMIN-UI-MODERNIZATION.md)
- [TASKS-ADMIN-UI-MODERNIZATION.md](../TASKS-ADMIN-UI-MODERNIZATION.md)
- UX gap plan (completed): `.cursor/plans/admin_ux_gap_analysis_6b47e72b.plan.md`
- Key files: `packages/ui/src/lib/components/Sidebar.tsx`,
  `packages/ui/src/lib/components/AppBar.tsx`,
  `packages/ui/src/lib/layouts/DashboardLayout.tsx`,
  `packages/theme/src/mui-theme.ts`,
  `apps/admin/src/constants/navItems.tsx`
