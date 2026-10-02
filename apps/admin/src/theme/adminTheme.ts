import { alpha, createTheme, darken, getContrastRatio, lighten } from '@mui/material/styles';
import type { Appearance } from './appearance';
import { defaultAppearance } from './appearance';

/** Admin-only design system. The kiosk retains its own theme. */
export function createAdminTheme(
  preferences: Appearance = defaultAppearance,
  mode: 'light' | 'dark' = 'light',
) {
  const dark = mode === 'dark';
  let accent = preferences.accent;
  const surface = dark ? '#1b2632' : '#ffffff';
  for (let i = 0; i < 20 && getContrastRatio(accent, surface) < 4.5; i++) {
    accent = dark ? lighten(accent, 0.12) : darken(accent, 0.12);
  }
  const radius = preferences.corners === 'square' ? 2 : 10;
  return createTheme({
    palette: {
      mode,
      contrastThreshold: 4.5,
      primary: { main: accent },
      secondary: { main: '#53667d' },
      background: { default: dark ? '#131c24' : '#f6f7f9', paper: dark ? '#1b2632' : '#ffffff' },
      text: { primary: dark ? '#ecf1f6' : '#172b37', secondary: dark ? '#a8b6c4' : '#667782' },
      divider: dark ? '#354452' : '#e5e9ed',
      success: { main: '#197454' },
      warning: { main: '#a56912' },
      error: { main: '#bd3f3f' },
      info: { main: '#3275b6' },
    },
    typography: {
      fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      fontSize: 13,
      h1: { fontSize: '2.25rem', fontWeight: 700, letterSpacing: '-.055em' },
      h2: { fontSize: '2rem', fontWeight: 700, letterSpacing: '-.045em' },
      h3: { fontSize: '1.8rem', fontWeight: 700, letterSpacing: '-.04em' },
      h4: { fontSize: '1.65rem', fontWeight: 700, letterSpacing: '-.035em' },
      h5: { fontSize: '1.2rem', fontWeight: 650, letterSpacing: '-.02em' },
      h6: { fontSize: '1rem', fontWeight: 650 },
      subtitle1: { fontWeight: 600 },
      button: { textTransform: 'none', fontWeight: 600, letterSpacing: 0 },
      body2: { lineHeight: 1.65 },
      overline: { fontSize: '.65rem', fontWeight: 700, letterSpacing: '.1em' },
    },
    shape: { borderRadius: radius },
    components: {
      MuiCssBaseline: {
        styleOverrides: {
          body: { margin: 0 },
          '*': { boxSizing: 'border-box' },
          '::selection': { background: alpha(preferences.accent, 0.25) },
        },
      },
      MuiButton: {
        defaultProps: { disableElevation: true },
        styleOverrides: {
          root: { borderRadius: radius, minHeight: 38, paddingInline: 16 },
          outlined: { borderColor: dark ? '#526475' : '#d5dfe0' },
        },
      },
      MuiCard: {
        defaultProps: { variant: 'outlined' },
        styleOverrides: { root: { backgroundImage: 'none', boxShadow: '0 2px 3px #172b3703' } },
      },
      MuiCardHeader: {
        styleOverrides: {
          root: { padding: '20px 24px' },
          title: { fontSize: '1rem', fontWeight: 650 },
          subheader: { marginTop: 4, fontSize: '.8rem' },
        },
      },
      MuiCardContent: {
        styleOverrides: {
          root: {
            padding: preferences.density === 'compact' ? 16 : 24,
            '&:last-child': { paddingBottom: preferences.density === 'compact' ? 16 : 24 },
          },
        },
      },
      MuiPaper: { styleOverrides: { root: { backgroundImage: 'none' } } },
      MuiTextField: { defaultProps: { size: 'small' } },
      MuiOutlinedInput: {
        styleOverrides: {
          root: { backgroundColor: dark ? '#1b2632' : '#fff', borderRadius: radius },
          notchedOutline: { borderColor: dark ? '#526475' : '#d6dee3' },
        },
      },
      MuiInputLabel: { styleOverrides: { root: { fontSize: '.85rem' } } },
      MuiTableCell: {
        styleOverrides: {
          head: {
            background: dark ? '#202c38' : '#f8fafb',
            fontSize: '.7rem',
            letterSpacing: '.035em',
            fontWeight: 650,
          },
          body: { fontSize: '.82rem' },
          root: {
            borderColor: dark ? '#354452' : '#edf0f2',
            padding: preferences.density === 'compact' ? '8px 12px' : '16px',
          },
        },
      },
      MuiChip: {
        defaultProps: { size: 'small' },
        styleOverrides: {
          root: { borderRadius: 6, fontWeight: 600, fontSize: '.7rem' },
          colorSuccess: { color: '#176b51', background: '#e7f4ed' },
          colorWarning: { color: '#895810', background: '#fff3d9' },
          colorError: { color: '#a83434', background: '#fcecec' },
          colorInfo: { color: '#2d659a', background: '#edf4fd' },
        },
      },
      MuiTab: {
        styleOverrides: { root: { textTransform: 'none', minHeight: 52, fontWeight: 600 } },
      },
      MuiDialog: {
        defaultProps: { fullWidth: true, maxWidth: 'sm' },
        styleOverrides: { paper: { borderRadius: radius } },
      },
      MuiAlert: {
        styleOverrides: {
          root: { borderRadius: radius },
          standardInfo: { background: dark ? '#203749' : '#eef4f9' },
        },
      },
      MuiTooltip: { defaultProps: { arrow: true } },
      MuiIconButton: { styleOverrides: { root: { borderRadius: radius } } },
      MuiLinearProgress: {
        styleOverrides: {
          root: { height: 6, borderRadius: 4, background: alpha(preferences.accent, 0.15) },
          bar: { borderRadius: 4 },
        },
      },
    },
  });
}
export const adminTheme = createAdminTheme();
