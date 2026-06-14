# @gaming-cafe/ui

Shared MUI-based React components for admin and future apps. See [PLAN-ADMIN-UI-MODERNIZATION.md](../../docs/PLAN-ADMIN-UI-MODERNIZATION.md) and [TASKS-ADMIN-UI-MODERNIZATION.md](../../docs/TASKS-ADMIN-UI-MODERNIZATION.md) for the migration roadmap.

## Layout primitives

- `PageShell` — outer page padding with optional `header`, `toolbar`, and `footer` slots
- `PageHeader` — title, description, breadcrumbs, and action row
- `ListPage` — composite list page: `PageShell` + header + optional filters + `DataGrid` + pagination

### PageShell (standalone)

Use `PageShell` when a page needs custom layout chrome without the list/form/detail composites — dashboards, POS, or multi-column bespoke flows.

```tsx
import { PageHeader, PageShell } from '@gaming-cafe/ui';

<PageShell
  dense
  maxWidth="lg"
  toolbar={<ToggleButtonGroup>...</ToggleButtonGroup>}
  footer={<Pagination ... />}
>
  <PageHeader title="Admin dashboard" description="Overview for today." />
  <Grid container spacing={3}>{/* stat cards, charts */}</Grid>
</PageShell>
```

Slots: `toolbar` (filters, date toggles), `footer` (pagination), `dense` / `maxWidth` for content width.

### When to compose vs extend

| Need | Use | Do not |
|------|-----|--------|
| Standard list | `ListPage` | Raw `DataGrid` + manual `PageShell` |
| Create/edit form | `FormPage` + `FormBuilder` | `Typography h4` + loose `Paper` |
| Entity detail | `DetailPage` | Duplicate header/summary chrome |
| Bespoke counter (POS) | `CounterSaleLayout` in admin (`containers/sales`) or `PageShell` | `FormPage` (single Paper fights layout) |
| Mobile list cards | `ListPage` + `mobileCardRender` | Custom page-only table fork |

Extend composites only when a prop gap is real — prefer composing primitives (`PageShell` + local sections) over forking package components.

```tsx
import { ListPage } from '@gaming-cafe/ui';
import { buildListUrl } from '../utils/buildListUrl';

<ListPage
  title="Sessions"
  description="Manage player gaming sessions."
  data={rows}
  columns={columns}
  actions={actions}
  isLoading={isLoading}
  filters={<FilterChips />}
  showSearch={false}
  onAddClick={handleAdd}
  addButtonLabel="Start New Session"
  pagination={{
    page,
    totalPages: data?.totalPages,
    onPageChange: (value) =>
      navigate(buildListUrl('/sessions', value, { active: activeParam ?? undefined })),
  }}
  mobileCardRender={(row, rowActions) => (
    <SessionListMobileCard row={row} actions={rowActions} />
  )}
/>
```

Filter query preservation stays in the app layer via `buildListUrl` — `ListPage` only calls `onPageChange(page)`.

Below `md`, `ListPage` renders stacked cards instead of a table. Pass `mobileCardRender` for a custom card layout; when omitted, a default label/value `ListMobileCard` is used. Desktop table layout is unchanged.

## Form pages

- `FormPage` — composite form page: `PageShell` + `PageHeader` + `Paper` card

```tsx
import { FormBuilder, FormPage } from '@gaming-cafe/ui';
import { ActiveShiftGuard } from '../components/ActiveShiftGuard';

<ActiveShiftGuard>
  <FormPage
    title="Start New Session"
    description="Start a new gaming session for a player"
    backTo="/sessions"
    backLabel="Back to sessions"
  >
    <FormBuilder fields={fields} schema={schema} onSubmit={handleSubmit} />
  </FormPage>
</ActiveShiftGuard>
```

Wrap with `ActiveShiftGuard` in admin when the flow requires an active shift. Alerts and auxiliary content go in `children` before `FormBuilder`.

### Form helper text

On `FormBuilder` fields, set **`helperText`** on each `FieldConfig` — one short sentence describing what the field controls and any constraint (e.g. `"Day price in ₹; charged during 8 AM – 11 PM"`).

```tsx
{
  name: 'price',
  label: 'Day price (₹)',
  type: 'number',
  helperText: 'Day price in ₹; charged during 8 AM – 11 PM venue time',
}
```

`FormBuilder` renders `helperText` below every supported field type (text, number, select, search, switch, etc.). Validation errors replace helper text when present. The legacy `formHelperText` prop is deprecated — use `helperText` only.

### Counter sale flows (admin)

Product POS and plan sell use **`CounterSaleLayout`** (`apps/admin/src/containers/sales/`) — shared payment tiles, player picker, and a 7/5 catalog + sticky summary grid. Use `FormPage` for config/create forms; use `CounterSaleLayout` for staff counter checkout flows.

## Detail pages

- `DetailPage` — composite detail view: `PageShell` + header + status chip + summary card + sections + actions
- `ErrorPanel` — fetch-failure message with optional Retry (used by `DetailPage` when `error` is set)

```tsx
import { DataGrid, DetailPage } from '@gaming-cafe/ui';

<DetailPage
  title="Shift details"
  backTo="/shifts"
  backLabel="Back to shifts"
  isLoading={isLoading}
  error={!isLoading && !shift ? 'Shift not found' : null}
  onRetry={() => refetch()}
  status={{ label: 'Active', color: 'success' }}
  summary={<SummaryGrid />}
  actions={<Button>Force close</Button>}
  sections={[
    {
      title: 'Entries',
      description: 'Related records',
      content: <DataGrid columns={columns} data={rows} />,
    },
  ]}
/>
```

Mutation dialogs (confirm, reconcile, etc.) stay in page code as siblings after `DetailPage`.

`DetailPage` renders `ErrorPanel` internally when the `error` prop is set.

## Feedback and loading

### Feedback components

- `EmptyState` — empty list or content area with optional primary CTA
- `ErrorPanel` — fetch failure message with optional Retry button

```tsx
import { EmptyState } from '@gaming-cafe/ui';

<EmptyState
  title="No sessions yet"
  description="Start a session when a player is ready."
  actionLabel="Start New Session"
  onAction={handleStartNewSession}
/>
```

`ListPage` renders `EmptyState` automatically when `data.length === 0` and not loading. Pass `emptyMessage` for the title; `onAddClick` / `addButtonLabel` become the empty-state CTA when set.

### Loading: which skeleton when

| Component | Use when | Avoid when |
|-----------|----------|------------|
| `GridSkeleton` | `ListPage` / table list initial load | Form or detail pages |
| `FormSkeleton` | `DetailPage`, form/detail pages loading entity data | Full-width data tables |
| `CircularProgress` | Inline or button loading, small guards (`ActiveShiftGuard`) | Full-page list or form chrome |

Prefer skeletons over centered full-page spinners for page-level loading.

Design tokens for semantic feedback colors: [ADR-0007](../../docs/adr/0007-design-tokens-shared.md).

## Notifications

Use `toastUtils` from `@gaming-cafe/utils` for all user-facing toasts. **Do not** import `toast` from `react-toastify` directly.

`ToastContainer` is mounted once in `@gaming-cafe/providers` (`Providers.tsx`). Do not add another container in app code.

```tsx
import { toastUtils } from '@gaming-cafe/utils';

toastUtils.success('Saved');
toastUtils.error('Something went wrong');
toastUtils.sessionWarning('Player has 5 minutes remaining. Tap to view session.', {
  sessionId: 'uuid',
  tag: 'session-uuid-5min',
});
```

| Method | Use when |
|--------|----------|
| `toastUtils.success` / `error` / `info` / `warning` | Standard feedback after mutations or validation |
| `toastUtils.sessionWarning` | Session countdown alerts (primary orange, clickable, longer duration) |
| `toastUtils.promise` | Async operations with pending/success/error messages |

Toast colors and position come from shared config in `@gaming-cafe/utils` (`toast-config.ts`) and token-aligned CSS in `@gaming-cafe/providers`.

## Deprecated patterns

| Pattern | Replacement |
|---------|-------------|
| `ListViewPage` | `ListPage` |
| Manual list `Box` + `Pagination` + raw table | `ListPage` |
| Full-page `CircularProgress` for list/form load | `GridSkeleton` / `FormSkeleton` |
| Direct `react-toastify` `toast` import | `toastUtils` from `@gaming-cafe/utils` |
| `@mui/material/GridLegacy` | `Grid` from `@mui/material` with `size={{ xs, sm, md }}` (MUI v7) |

## Export index

Public exports from `packages/ui/src/index.ts`:

- **Layout:** `PageShell`, `PageHeader`, `AuthLayout`, `DashboardLayout`, `AppBar`, `Sidebar`
- **Page composites:** `ListPage`, `FormPage`, `DetailPage`, `CrudPage` (legacy)
- **Data display:** `DataGrid`, `EmptyState`, `ErrorPanel`, `GameCard`, `IconFallback`
- **Forms:** `FormBuilder`, `useFormBuilder`, `forms/*`
- **Loading:** `GridSkeleton`, `FormSkeleton`
- **Deprecated:** `ListViewPage` — use `ListPage`

Types re-exported alongside each component (e.g. `ListPageProps`, `Column`, `Action`, `DetailPageSection`).

## References

- [ADR-0007: Design tokens shared](../../docs/adr/0007-design-tokens-shared.md)
- [PLAN: Admin UI modernization](../../docs/PLAN-ADMIN-UI-MODERNIZATION.md)
- [TASKS: Admin UI modernization](../../docs/TASKS-ADMIN-UI-MODERNIZATION.md)
- [VISUAL-QA-ADMIN.md](../../docs/VISUAL-QA-ADMIN.md) — manual regression checklist

## Peer dependencies

- `react@^19`
- `@mui/material@^7`
- `@gaming-cafe/theme` (workspace)
