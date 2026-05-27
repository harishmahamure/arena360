# @gaming-cafe/theme

Design-token bridge for Arena360. Exposes the same palette / spacing / typography in two shapes so admin (MUI) and kiosk (CSS variables) can share a brand without one rewriting the other:

- `tokens` — strongly-typed TS object, the canonical source.
- `muiTheme` — pre-built `@mui/material` `Theme` derived from `tokens` for the admin SPA.
- `@gaming-cafe/theme/tokens.css` — CSS custom properties (`--gz-color-*`, `--gz-space-*`, …) imported by the kiosk.

See `docs/adr/0007-design-tokens-shared.md` for the why.

## Usage

Admin (MUI):

```ts
import { ThemeProvider } from '@mui/material/styles';
import { muiTheme } from '@gaming-cafe/theme';

<ThemeProvider theme={muiTheme}>{children}</ThemeProvider>;
```

Kiosk (CSS vars):

```ts
import '@gaming-cafe/theme/tokens.css';
```

```css
.button {
  background: var(--gz-color-primary);
  color: var(--gz-color-on-primary);
  border-radius: var(--gz-radius-md);
  padding: var(--gz-space-3) var(--gz-space-5);
}
```
