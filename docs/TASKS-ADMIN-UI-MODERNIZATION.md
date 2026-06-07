# TASKS: Admin UI Modernization

> Atomic, ordered tasks for [PLAN-ADMIN-UI-MODERNIZATION.md](PLAN-ADMIN-UI-MODERNIZATION.md).
> Conventional commit scope: `ui(admin)` or `ui(theme)`.
>
> Last updated: 2026-06-07

## Overview

Modernize admin visual system and page patterns without changing backend APIs
or user workflows. Work spans `packages/theme`, `packages/ui`, and `apps/admin`.

---

## Milestone M1 — Foundation

### UI-001: Consolidate MUI theme onto tokens

**Type:** UI / Theme
**Complexity:** M
**Priority:** Must
**Status:** `done`
**Blocked by:** —
**Blocks:** UI-002, UI-003, UI-004, UI-005

#### Description

Merge useful component overrides from `packages/theme/src/lib/theme.ts` into
`packages/theme/src/mui-theme.ts` (built from `tokens.ts`). Wire
`packages/providers` to export the token-based theme. Deprecate `lib/theme.ts`
with a comment pointing to `mui-theme.ts`.

#### Acceptance criteria

- [x] `Providers` imports `muiTheme` from `@gaming-cafe/theme` (not `lib/theme.ts`)
- [x] Primary, secondary, semantic colors match `tokens.ts` / `tokens.css`
- [x] Button, Card, AppBar, TextField overrides preserved or improved
- [x] Login (`AuthLayout` dark theme) still renders correctly
- [x] `pnpm --filter @gaming-cafe/ui build` and admin typecheck pass

#### Key files

- `packages/theme/src/mui-theme.ts`
- `packages/theme/src/lib/theme.ts`
- `packages/theme/src/tokens.ts`
- `packages/providers/src/lib/Providers.tsx`

---

### UI-002: PageShell primitive

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-001
**Blocks:** UI-008, UI-009, UI-010, UI-012+

#### Description

Add `PageShell` to `@gaming-cafe/ui`: consistent outer padding, optional
`maxWidth`, slots for header toolbar and footer (pagination). Integrate with
existing `PageHeader` in admin or move `PageHeader` into `packages/ui`.

#### Acceptance criteria

- [x] `PageShell` exported from `@gaming-cafe/ui`
- [x] Props: `children`, `maxWidth?`, `header?`, `toolbar?`, `footer?`
- [x] Responsive padding: `xs: 2`, `md: 4` horizontal; `xs: 2`, `md: 3` vertical
- [x] Adopted on 4 pilot pages (sessions list, staff dashboard, plan sales list,
      session detail; login excluded)

#### Key files

- `packages/ui/src/lib/components/PageShell.tsx` (new)
- `apps/admin/src/components/PageHeader.tsx`
- Pilot pages listed in PLAN

---

### UI-003: Unify toast API and styling

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-001
**Blocks:** —

#### Description

Standardize on `toastUtils` from `@gaming-cafe/utils`. Theme `ToastContainer` in
`Providers` to match palette (position, colors, duration). Replace direct
`react-toastify` imports in admin pages touched by this effort (incremental OK
in M1; complete by end of M4).

#### Acceptance criteria

- [x] Single `ToastContainer` config in `Providers`
- [x] Session warning toasts remain visually distinct (icon/color)
- [x] No new direct `toast` imports in pilot pages
- [x] Document preferred API in `packages/ui` README

#### Key files

- `packages/providers/src/lib/Providers.tsx`
- `apps/admin/src/hooks/useCountDown.ts`
- `apps/admin/src/hooks/useNotification.ts`

---

### UI-004: Semantic stat colors

**Type:** UI
**Complexity:** S
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-001
**Blocks:** UI-016

#### Description

Replace hardcoded hex in `StatCard` and `AdminDashboardView` with theme semantic
colors (`success.main`, `info.main`, etc.) or token references.

#### Acceptance criteria

- [x] No `#10B981`, `#8B5CF6`, etc. in dashboard stat definitions
- [x] Stat cards use theme palette or `tokens.color.*`
- [x] Visual appearance acceptable on pilot review

#### Key files

- `apps/admin/src/containers/stats/StatCard.tsx`
- `apps/admin/src/pages/dashboard/AdminDashboardView.tsx`

---

## Milestone M2 — App shell

### UI-005: Sidebar v2

**Type:** UI
**Complexity:** M
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-001
**Blocks:** UI-007

#### Description

Refresh sidebar: token-based active state (left accent bar + subtle background),
remove unused e-commerce `defaultNavItems`, improve collapsed mode with tooltips
and flyout popover for child routes.

#### Acceptance criteria

- [x] Active nav item uses `primary.main` accent, not hardcoded `rgba(255,105,0,...)`
- [x] Collapsed drawer: icon tooltips on all items
- [x] Collapsed drawer: child routes accessible via flyout/popover
- [x] Admin `navItems` unchanged in path structure
- [x] Mobile drawer behavior unchanged or improved

#### Key files

- `packages/ui/src/lib/components/Sidebar.tsx`
- `apps/admin/src/constants/navItems.tsx`

---

### UI-006: AppBar v2

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-005
**Blocks:** UI-007

#### Description

Contextual page title from route map; shift status chip for staff when active
shift exists; preserve permission + shift-gated POS/plan quick actions.

#### Acceptance criteria

- [x] AppBar shows current page title (derived from route → title map)
- [x] Staff see shift active/inactive indicator
- [x] Quick actions still respect permissions and `ActiveShiftGuard` rules
- [x] No regression on profile menu / logout / settings link

#### Key files

- `packages/ui/src/lib/components/AppBar.tsx`
- `apps/admin/src/layouts/DashboardLayout.tsx`

---

### UI-007: Nav grouping (rush-hour vs config)

**Type:** UI
**Complexity:** S
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-005
**Blocks:** —

#### Description

Visually group sidebar sections: Operations (Sessions, POS, Plan sales, Running
tab) vs Configuration (Products, Plans, Devices) vs Finance (Cash, Expenses) vs
System (Settings). Use `Divider` + section labels without changing routes.

#### Acceptance criteria

- [x] At least 3 labeled sections in `adminNavItems`
- [x] Permission filtering still works per item
- [x] Collapsed mode: section labels hidden gracefully

#### Key files

- `apps/admin/src/constants/navItems.tsx`
- `packages/ui/src/lib/components/Sidebar.tsx` (section label support if needed)

---

## Milestone M3 — Page pattern library

### UI-008: ListPage composite

**Type:** UI
**Complexity:** M
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002
**Blocks:** UI-012, UI-013, UI-014

#### Description

Compose `PageShell` + title/description + optional `FilterBar` slot + `DataGrid` +
`PaginationFooter`. Generalize sessions filter chips pattern.

#### Acceptance criteria

- [x] `ListPage<T>` exported from `@gaming-cafe/ui`
- [x] Supports: `columns`, `actions`, `data`, `isLoading`, `filters` slot,
      `onPageChange` with query preservation callback
- [x] `showSearch` prop retained (default false until API wired)
- [x] `DataGrid` styling updated: softer headers, consistent row hover

#### Key files

- `packages/ui/src/lib/components/ListPage/index.tsx` (new)
- `packages/ui/src/lib/components/DataGrid.tsx`
- `packages/ui/src/lib/components/ListViewPage/index.tsx` (deprecate or wrap)

---

### UI-009: FormPage shell

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002
**Blocks:** UI-014 (partial)

#### Description

Standard form page: `PageHeader` + back link + `Paper` card with consistent
padding + `FormBuilder` slot. Replace ad-hoc `Typography h4` + `Paper p:4` on
create/edit pages.

#### Acceptance criteria

- [x] `FormPage` component with `title`, `backTo`, `breadcrumbs?`, `children`
- [x] Adopted on: `SessionNewPage`, `PlanTransactionNewPage`, `PlayerNewPage`
- [x] `ActiveShiftGuard` wrappers preserved where present

#### Key files

- `packages/ui/src/lib/components/FormPage.tsx` (new)
- `apps/admin/src/pages/dashboard/sessions/SessionNewPage.tsx`
- `apps/admin/src/pages/dashboard/plan-transactions/PlanTransactionNewPage.tsx`

---

### UI-010: DetailPage shell

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002
**Blocks:** UI-018

#### Description

Standard detail page: `PageHeader` + status chip row + summary card + section
slots. Unify shift/cash/session detail chrome.

#### Acceptance criteria

- [x] `DetailPage` component with `summary`, `sections[]`, `actions?` slots
- [x] Loading uses `FormSkeleton`; error uses `ErrorPanel`
- [x] Adopted on `ShiftDetailPage` and `CashRegisterDetailPage`

#### Key files

- `packages/ui/src/lib/components/DetailPage.tsx` (new)
- `apps/admin/src/pages/dashboard/shifts/ShiftDetailPage.tsx`
- `apps/admin/src/pages/dashboard/cash-registers/CashRegisterDetailPage.tsx`

---

### UI-011: EmptyState + ErrorPanel + loading kit

**Type:** UI
**Complexity:** S
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002
**Blocks:** UI-012+, UI-015, UI-016

#### Description

Shared feedback components: empty list with primary CTA, error with Retry,
standard skeleton usage guidelines.

#### Acceptance criteria

- [x] `EmptyState`: icon, title, description, `actionLabel` + `onAction`
- [x] `ErrorPanel`: message + optional Retry button
- [x] `ListPage` renders `EmptyState` when `data.length === 0` and not loading
- [x] README snippet for when to use `GridSkeleton` vs `FormSkeleton` vs spinner

#### Key files

- `packages/ui/src/lib/components/EmptyState.tsx` (new)
- `packages/ui/src/lib/components/ErrorPanel.tsx` (new)
- `packages/ui/README.md`

---

## Milestone M4 — List migration

### UI-012: List wave 1 — high-traffic pages

**Type:** UI
**Complexity:** L
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-008, UI-011
**Blocks:** UI-020

#### Description

Migrate to `ListPage`: Sessions, Players, Plan sales, POS sales.

#### Acceptance criteria

- [x] All four pages use `ListPage` + `PageShell` (Sessions migrated in UI-008; `ListPage` includes `PageShell`)
- [x] Existing filters, chips, pagination query params preserved
- [x] `hideOnMobile` columns retained or improved
- [x] No functional regression (permissions, actions, navigation)

#### Key files

- `apps/admin/src/pages/dashboard/sessions/SessionsPage.tsx`
- `apps/admin/src/pages/dashboard/players/PlayersPage.tsx`
- `apps/admin/src/pages/dashboard/plan-transactions/PlanTransactionsPage.tsx`
- `apps/admin/src/pages/dashboard/product-transactions/ProductTransactionsPage.tsx`

---

### UI-013: List wave 2 — money flows

**Type:** UI
**Complexity:** M
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-012
**Blocks:** —

#### Description

Migrate to `ListPage`: Shifts, Cash registers, Cash deposits.

#### Acceptance criteria

- [x] All three pages use `ListPage`
- [x] Force-close / approve actions unchanged
- [x] Pagination and filters preserved

#### Key files

- `apps/admin/src/pages/dashboard/shifts/ShiftsPage.tsx`
- `apps/admin/src/pages/dashboard/cash-registers/CashRegistersPage.tsx`
- `apps/admin/src/pages/dashboard/cash-deposits/CashDepositsPage.tsx`

---

### UI-014: List wave 3 — config and inventory

**Type:** UI
**Complexity:** L
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-013
**Blocks:** —

#### Description

Migrate remaining list pages: Products, Plans, Devices, Games, Units, Vendors,
Expenses, Inventory locations/transfers/waste.

#### Acceptance criteria

- [x] All listed pages use `ListPage` or documented exception (also: warehouse stock, waste report)
- [x] `showSearch={false}` until API search exists (pages with API or client search keep `showSearch`)
- [x] Biome clean on touched files

#### Key files

- `apps/admin/src/pages/dashboard/products/ProductsPage.tsx`
- `apps/admin/src/pages/dashboard/plans/PlansPage.tsx`
- `apps/admin/src/pages/dashboard/devices/DevicesPage.tsx`
- (and remaining list pages under `apps/admin/src/pages/dashboard/`)

---

## Milestone M5 — Bespoke screens

### UI-015: Staff dashboard visual refresh

**Type:** UI
**Complexity:** M
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002, UI-011
**Blocks:** —

#### Description

Modern staff home: hero shift banner, large quick-action row, optional
"sessions ending soon" strip fed from active session query, empty state when no
shift.

#### Acceptance criteria

- [x] Quick actions min 44px touch height
- [x] Shift duration and collection summary in prominent card
- [x] Uses semantic colors only
- [x] ≤2 taps to start session / sell / buy plan

#### Key files

- `apps/admin/src/pages/dashboard/StaffDashboardView.tsx`

---

### UI-016: Admin dashboard visual refresh

**Type:** UI
**Complexity:** M
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-004, UI-011
**Blocks:** —

#### Description

Owner dashboard: segmented Today / 7d / MTD control, stat grid with `StatCard`,
top performers cards with consistent spacing, device utilization without dead
chrome.

#### Acceptance criteria

- [x] Date range as segmented control (not loose buttons)
- [x] Stat grid responsive 1 → 2 → 3 columns
- [x] No non-functional menus/icons
- [x] Loading skeleton instead of full-page spinner

#### Key files

- `apps/admin/src/pages/dashboard/AdminDashboardView.tsx`
- `apps/admin/src/containers/stats/StatCard.tsx`

---

### UI-017: POS screen refresh

**Type:** UI
**Complexity:** L
**Priority:** Must
**Status:** `done`
**Blocked by:** UI-002
**Blocks:** —

#### Description

Visual-only modernization of `ProductTransactionNewPage`: product grid cards,
sticky cart summary, payment method as large selectable tiles. No cart/pricing
logic changes.

#### Acceptance criteria

- [x] Product selection grid uses card layout with clear selected state
- [x] Cart visible without excessive scroll on 1280px
- [x] Payment methods visually distinct (cash / online / split)
- [x] `ActiveShiftGuard` preserved

#### Key files

- `apps/admin/src/pages/dashboard/product-transactions/ProductTransactionNewPage.tsx`

---

### UI-018: Session detail refresh

**Type:** UI
**Complexity:** M
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-010
**Blocks:** —

#### Description

Session detail: timeline-style sections, player/device hero, sticky action bar on
mobile, TOTP dialog styling aligned with design system.

#### Acceptance criteria

- [x] Uses `DetailPage` shell
- [x] End / force-end / buy-more-time actions visible on mobile
- [x] Remaining time prominently displayed for active sessions

#### Key files

- `apps/admin/src/pages/dashboard/sessions/SessionDetailPage.tsx`
- `apps/admin/src/components/StaffTotpDialog.tsx`

---

### UI-019: Shift handover dialog polish

**Type:** UI
**Complexity:** S
**Priority:** Could
**Status:** `done`
**Blocked by:** UI-001
**Blocks:** —

#### Description

Stepper or clearer section layout for reconciliation summary; consistent
spacing and typography with new theme.

#### Acceptance criteria

- [x] No `window.location.reload()` (already removed — verify still true)
- [x] Dialog matches card radius and typography scale
- [x] TOTP field uses same pattern as `StaffTotpDialog`

#### Key files

- `apps/admin/src/components/ShiftHandoverDialog.tsx`

---

## Milestone M6 — Polish

### UI-020: Mobile card-list fallback (sessions)

**Type:** UI
**Complexity:** M
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-012
**Blocks:** —

#### Description

Below `sm` breakpoint, render sessions as stacked cards (player, device, time
left, status, actions) instead of horizontal table scroll.

#### Acceptance criteria

- [x] `SessionsPage` readable at 375px without horizontal scroll
- [x] Card actions match table row actions
- [x] Desktop table view unchanged

#### Key files

- `apps/admin/src/pages/dashboard/sessions/SessionsPage.tsx`
- `packages/ui/src/lib/components/DataGrid.tsx` (optional `mobileRender`)

---

### UI-021: GridLegacy → Grid2 migration

**Type:** UI / Refactor
**Complexity:** M
**Priority:** Could
**Status:** `done`
**Blocked by:** —
**Blocks:** —

#### Description

Replace `@mui/material/GridLegacy` with `Grid2` across admin (~9 files).

#### Acceptance criteria

- [x] No `GridLegacy` imports remain in `apps/admin`
- [x] Layout visually equivalent on pilot pages
- [x] Typecheck passes

---

### UI-022: packages/ui pattern documentation

**Type:** DOC
**Complexity:** S
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-008, UI-009, UI-010, UI-011
**Blocks:** —

#### Description

Author `packages/ui/README.md`: PageShell, ListPage, FormPage, DetailPage,
EmptyState, when to extend vs compose.

#### Acceptance criteria

- [x] README with import examples
- [x] Links to ADR-0007 and PLAN doc
- [x] List of deprecated patterns (`ListViewPage` direct use → `ListPage`)

---

### UI-023: Visual regression checklist

**Type:** QA
**Complexity:** S
**Priority:** Should
**Status:** `done`
**Blocked by:** UI-012, UI-015, UI-017
**Blocks:** —

#### Description

Document manual visual QA checklist at 375 / 768 / 1280 for pilot pages and
staff loop.

#### Acceptance criteria

- [x] Checklist in `docs/` or `apps/admin/README.md`
- [x] Covers: login, staff dashboard, sessions list/detail, POS, plan purchase
- [ ] Signed off once after M5 complete (manual pass pending)

---

## Task index (quick reference)

| ID | Title | Size | Priority | Milestone |
|----|-------|------|----------|-----------|
| UI-001 | Consolidate MUI theme onto tokens | M | Must | M1 |
| UI-002 | PageShell primitive | S | Must | M1 |
| UI-003 | Unify toast API and styling | S | Must | M1 |
| UI-004 | Semantic stat colors | S | Should | M1 |
| UI-005 | Sidebar v2 | M | Must | M2 |
| UI-006 | AppBar v2 | S | Must | M2 |
| UI-007 | Nav grouping | S | Should | M2 |
| UI-008 | ListPage composite | M | Must | M3 |
| UI-009 | FormPage shell | S | Must | M3 |
| UI-010 | DetailPage shell | S | Must | M3 |
| UI-011 | EmptyState + ErrorPanel | S | Must | M3 |
| UI-012 | List wave 1 | L | Must | M4 |
| UI-013 | List wave 2 | M | Must | M4 |
| UI-014 | List wave 3 | L | Should | M4 |
| UI-015 | Staff dashboard refresh | M | Must | M5 |
| UI-016 | Admin dashboard refresh | M | Should | M5 |
| UI-017 | POS screen refresh | L | Must | M5 |
| UI-018 | Session detail refresh | M | Should | M5 |
| UI-019 | Shift handover polish | S | Could | M5 |
| UI-020 | Mobile session cards | M | Should | M6 |
| UI-021 | Grid2 migration | M | Could | M6 |
| UI-022 | UI package docs | S | Should | M6 |
| UI-023 | Visual regression checklist | S | Should | M6 |

---

## Definition of done (all tasks)

- [ ] Code follows Biome format/lint (ADR-0006)
- [ ] No new hardcoded brand hex outside tokens
- [ ] `pnpm --filter @gaming-cafe/ui build` if `packages/ui` changed
- [ ] Admin typecheck passes for touched routes
- [ ] Acceptance criteria checked
- [ ] Conventional commit: `feat(ui): …` or `refactor(ui): …`
