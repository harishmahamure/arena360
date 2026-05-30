/**
 * Arena360 design tokens.
 *
 * Single source of truth for color, spacing, radius, typography, and
 * breakpoint values. Consumed by `mui-theme.ts` (admin) and by the
 * companion `tokens.css` (kiosk). Token names match the CSS custom
 * properties in `tokens.css` so swapping a kiosk page to MUI later is
 * mechanical.
 *
 * Palette values are inherited from the legacy
 * `libs/shared/theme/src/lib/theme.ts` to keep the brand consistent
 * during the consolidation. A token-rename should land as a single PR
 * with both this file and `tokens.css` updated together.
 */

export const tokens = {
  color: {
    primary: '#FF6900',
    primaryLight: '#FF8A3D',
    primaryDark: '#CC5400',
    onPrimary: '#FFFFFF',

    secondary: '#1A1A2E',
    secondaryLight: '#2D2D44',
    secondaryDark: '#0F0F1A',
    onSecondary: '#FFFFFF',

    surface: '#FFFFFF',
    surfaceAlt: '#F8F9FC',
    onSurface: '#18181B',
    onSurfaceMuted: '#52525B',

    success: '#10B981',
    warning: '#F59E0B',
    danger: '#EF4444',
    info: '#3B82F6',

    border: '#E4E4E7',
    divider: '#E4E4E7',
  },

  /**
   * Dark surface scale (ggCircuit-style). Slate backgrounds with the same
   * orange accent. Consumed by `muiDarkTheme` (admin login) and mirrored as
   * the `--gz-dark-*` custom properties in `tokens.css` (kiosk shell).
   */
  darkColor: {
    surface: '#141A2A',
    surfaceAlt: '#1C2436',
    surfaceRaised: '#222C42',
    background: '#0B0F1A',
    onSurface: '#E8ECF4',
    onSurfaceMuted: '#94A3B8',
    border: '#2A3550',
    divider: '#2A3550',
  },

  space: {
    1: '4px',
    2: '8px',
    3: '12px',
    4: '16px',
    5: '20px',
    6: '24px',
    7: '32px',
    8: '40px',
  },

  radius: {
    sm: '8px',
    md: '12px',
    lg: '16px',
  },

  typography: {
    fontSans: '"Zalando Sans", Roboto, Arial, sans-serif',
    fontMono: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace',
  },

  breakpoint: {
    xs: 0,
    sm: 640,
    md: 768,
    lg: 1024,
    xl: 1280,
  },
} as const;

export type Tokens = typeof tokens;
