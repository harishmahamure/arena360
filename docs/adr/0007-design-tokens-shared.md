# ADR-0007: Shared design tokens via `packages/theme`

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

The admin SPA (`apps/admin`, post-migration) is built on **MUI** and
consumes a real `@mui/material` `Theme` object exposed today by
`game-zone-management-fe/libs/shared/theme/`.

The Tauri kiosk (`apps/kiosk`) is **not** built on MUI. It ships its
own bespoke CSS, hand-tuned for the kiosk's fullscreen,
keyboard-driven UX. Replacing it with MUI would be a multi-week UI
rewrite for no immediate user-visible benefit; the existing kiosk UI
works, it just has no shared visual contract with admin.

Today the two surfaces share **nothing visually**: admin's primary
colour is set in the MUI theme, kiosk's primary colour is a hex
literal in a `.css` file. A brand refresh requires touching both
codebases independently.

We need a way to share the **design tokens** (colour, typography,
spacing, radii, shadows) between admin and kiosk without forcing a UI
rewrite on the kiosk.

## Decision

`packages/theme` becomes the **single source of truth for design
tokens** and exposes them in **two simultaneous forms**:

1. **A TypeScript token map and an MUI `Theme`**, consumed by admin:

   ```ts
   import { tokens, muiTheme } from '@gaming-cafe/theme';

   // tokens: typed object — { color: { bg, fg, accent, danger }, spacing: { 1..8 }, ... }
   // muiTheme: a full @mui/material Theme built from tokens
   ```

2. **A generated `tokens.css` file** with CSS custom properties,
   consumed by the kiosk via a single `import` in `main.tsx`:

   ```ts
   import '@gaming-cafe/theme/dist/tokens.css';
   ```

   `tokens.css` is produced at build time by
   `packages/theme/scripts/emit-tokens-css.ts`, which walks the same
   `tokens` object that the MUI theme is built from. The two
   representations cannot drift because they share one source.

The naming convention for the CSS variables is **`--gz-<category>-<name>`**
to keep them unmistakable in DevTools and avoid collisions with MUI's
own CSS-in-JS classnames:

```
--gz-color-bg, --gz-color-fg, --gz-color-accent, --gz-color-danger,
--gz-color-success, --gz-color-warning, --gz-color-muted,
--gz-radius-sm, --gz-radius-md, --gz-radius-lg, --gz-radius-pill,
--gz-spacing-1, --gz-spacing-2, --gz-spacing-3, --gz-spacing-4,
--gz-spacing-5, --gz-spacing-6, --gz-spacing-7, --gz-spacing-8,
--gz-font-sans, --gz-font-mono,
--gz-font-size-xs, --gz-font-size-sm, --gz-font-size-md,
--gz-font-size-lg, --gz-font-size-xl, --gz-font-size-2xl,
--gz-shadow-1, --gz-shadow-2, --gz-shadow-3
```

**Migration rule for kiosk CSS**: hex literals and magic numbers are
replaced over time with `var(--gz-*)` references. This is a gradual,
file-by-file refactor — not a rewrite. The first import of
`tokens.css` in `main.tsx` is the only mandatory change.

**Migration rule for admin**: continue to consume `muiTheme` exactly
as today. No change to component code.

## Consequences

### Positive

- **One brand source.** A primary-colour change in `tokens.ts`
  propagates to both surfaces on the next build.
- **No kiosk UI rewrite.** Kiosk keeps its bespoke CSS today; we
  trade hex literals for CSS variables incrementally as files are
  touched for other reasons.
- **MUI stays first-class on admin.** No "lowest-common-denominator"
  flattening of admin's component story.
- **Future-proof path to MUI on kiosk**, if and when we want it: the
  tokens are already authoritative, so swapping a custom button for
  an MUI `Button` is a per-component decision, not a system-wide
  rewrite.
- **DevTools-friendly.** The `--gz-` prefix makes shared tokens
  immediately recognisable when inspecting the kiosk in WebView.

### Negative

- **Two consumption mechanisms.** Slightly more surface area in
  `packages/theme` — TS exports plus a generated `.css` file. The
  generator script is ~50 lines and tested.
- **Token coverage gap.** Some MUI internals (e.g. component-level
  z-index scales) are not in the shared token map and don't appear
  as CSS variables. Acceptable: the kiosk doesn't need them; admin
  uses them via MUI directly.
- **No automatic enforcement on kiosk**. The kiosk could still ship
  raw hex literals after this ADR; we rely on PR review (and a future
  Biome rule banning hex colour literals in `apps/kiosk/**/*.css`)
  to keep the migration moving.

### Risks

- **Risk**: the TS tokens and the emitted CSS diverge.
  **Mitigation**: only the generator script writes `tokens.css`;
  the file is regenerated on every `packages/theme` build; a unit
  test asserts that every documented token name appears in the
  emitted CSS (ADR-0005 smoke test for `packages/theme`).
- **Risk**: a kiosk CSS rule overrides a token deliberately and
  the override is lost on a global token change.
  **Mitigation**: encourage scoped overrides (`var(--gz-color-bg,
  fallback)`) rather than hard-coded literals; flag in review.
- **Risk**: MUI updates change `Theme` shape and break `muiTheme`
  construction.
  **Mitigation**: MUI is pinned at a known good major in the admin
  `package.json`; upgrades happen in dedicated PRs.

## Alternatives considered

### Kiosk adopts MUI in full

- Pros: one component system across both surfaces; richer kiosk UI
  out of the box.
- Cons: multi-week rewrite for no immediate user benefit; MUI's
  default styles are desktop-app-flavoured and the kiosk is a
  fullscreen kiosk experience; adds MUI's runtime weight to the
  kiosk's already-light bundle.
- **Why rejected**: cost without justification today. A future ADR
  can revisit if we decide to invest in MUI everywhere.

### Tailwind in both apps

- Pros: utility-first; widely loved.
- Cons: admin is already MUI-shaped; mixing Tailwind with MUI on
  admin is a known pain; rewrites all of admin's styling.
- **Why rejected**: trades one inconsistency for another.

### Per-app independent themes, kept in sync by convention

- Pros: zero new packages.
- Cons: this is the failing status quo.
- **Why rejected**: convention has not held.

### Tokens emitted to JSON, consumed via Style Dictionary

- Pros: more output formats (iOS/Android too) if we ever need them.
- Cons: another tool; we have no native mobile clients; CSS + TS is
  sufficient.
- **Why rejected**: over-engineered for our consumer set.

## Implementation notes

- `packages/theme` exports:
  - `tokens: Tokens` — typed source map.
  - `muiTheme: Theme` — built from `tokens` via
    `@mui/material/styles/createTheme`.
  - `'@gaming-cafe/theme/dist/tokens.css'` — emitted at build by
    `scripts/emit-tokens-css.ts`.
- The build script reads `src/tokens.ts`, walks the object, and
  prints `:root { --gz-color-bg: <hex>; ... }` to
  `dist/tokens.css`.
- A Vitest test in `packages/theme/src/__tests__/tokens.test.ts`
  asserts every key in `tokens.color`, `tokens.spacing`,
  `tokens.radius`, `tokens.fontSize`, and `tokens.shadow` appears as
  a `--gz-*` custom property in the emitted CSS (this is the
  workspace's smoke test per ADR-0005).
- Kiosk migration: `apps/kiosk/src/main.tsx` imports
  `'@gaming-cafe/theme/dist/tokens.css'` once. Existing kiosk CSS
  is left alone; new and touched CSS uses `var(--gz-*)`.
- Admin migration: continues to consume `muiTheme` — no code change
  beyond the package rename (`@admin-panel/shared-theme` →
  `@gaming-cafe/theme`).

### Long-term roadmap (not in this ADR)

- Phase A (now, this ADR): tokens shared; admin unchanged; kiosk
  imports `tokens.css`.
- Phase B (future ADR): introduce shared **primitive** components
  in `packages/ui` that are MUI-free (buttons, inputs) and consume
  tokens directly via CSS variables. Both apps can use them.
- Phase C (future ADR, if justified): migrate kiosk to MUI piecewise.

This ADR commits only to Phase A. Phases B and C are mentioned for
context, not decided.

## References

- ADR-0001: Turborepo + pnpm (where `packages/theme` lives).
- ADR-0005: Testing strategy (`packages/theme` smoke test).
- ADR-0006: Biome (future colour-literal lint rule will be enforced
  here).
- `docs/ARCHITECTURE.md` — "Package responsibilities" table entry for
  `@gaming-cafe/theme`.
