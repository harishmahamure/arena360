import { createTheme, type Theme } from '@mui/material/styles';
import { tokens } from './tokens.js';

/**
 * Builds the MUI `Theme` from the shared design tokens. Admin imports
 * this; kiosk uses `tokens.css` instead. Keep the mapping mechanical —
 * any new visual decision should land as a token first, then surface
 * through here.
 */
export const muiTheme: Theme = createTheme({
  palette: {
    mode: 'light',
    primary: {
      main: tokens.color.primary,
      light: tokens.color.primaryLight,
      dark: tokens.color.primaryDark,
      contrastText: tokens.color.onPrimary,
    },
    secondary: {
      main: tokens.color.secondary,
      light: tokens.color.secondaryLight,
      dark: tokens.color.secondaryDark,
      contrastText: tokens.color.onSecondary,
    },
    success: { main: tokens.color.success },
    warning: { main: tokens.color.warning },
    error: { main: tokens.color.danger },
    info: { main: tokens.color.info },
    background: {
      default: tokens.color.surfaceAlt,
      paper: tokens.color.surface,
    },
    text: {
      primary: tokens.color.onSurface,
      secondary: tokens.color.onSurfaceMuted,
    },
    divider: tokens.color.divider,
  },
  typography: {
    fontFamily: tokens.typography.fontSans,
  },
  shape: {
    borderRadius: Number.parseInt(tokens.radius.md, 10),
  },
  breakpoints: {
    values: tokens.breakpoint,
  },
});
