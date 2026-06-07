import {
  alpha,
  createTheme,
  responsiveFontSizes,
  type Theme,
  type ThemeOptions,
} from '@mui/material/styles';
import { tokens } from './tokens.js';

/**
 * Grey scale derived from legacy theme — candidate for tokens in a follow-up.
 * Values aligned with `tokens.color.onSurface` / `divider` for zero visual drift.
 */
const paletteGrey = {
  50: '#FAFAFA',
  100: '#F4F4F5',
  200: tokens.color.divider,
  300: '#D4D4D8',
  400: '#A1A1AA',
  500: '#71717A',
  600: tokens.color.onSurfaceMuted,
  700: '#3F3F46',
  800: '#27272A',
  900: tokens.color.onSurface,
} as const;

const semanticPalette = {
  success: {
    main: tokens.color.success,
    light: '#34D399',
    dark: '#059669',
    contrastText: tokens.color.onPrimary,
  },
  warning: {
    main: tokens.color.warning,
    light: '#FBBF24',
    dark: '#D97706',
    contrastText: tokens.color.secondary,
  },
  error: {
    main: tokens.color.danger,
    light: '#F87171',
    dark: '#DC2626',
    contrastText: tokens.color.onPrimary,
  },
  info: {
    main: tokens.color.info,
    light: '#60A5FA',
    dark: '#2563EB',
    contrastText: tokens.color.onPrimary,
  },
} as const;

const fontFamily = tokens.typography.fontSans;

const themeShadows: ThemeOptions['shadows'] = [
  'none',
  '0px 1px 2px rgba(0, 0, 0, 0.05)',
  '0px 1px 3px rgba(0, 0, 0, 0.1), 0px 1px 2px rgba(0, 0, 0, 0.06)',
  '0px 4px 6px -1px rgba(0, 0, 0, 0.1), 0px 2px 4px -1px rgba(0, 0, 0, 0.06)',
  '0px 10px 15px -3px rgba(0, 0, 0, 0.1), 0px 4px 6px -2px rgba(0, 0, 0, 0.05)',
  '0px 20px 25px -5px rgba(0, 0, 0, 0.1), 0px 10px 10px -5px rgba(0, 0, 0, 0.04)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
  '0px 25px 50px -12px rgba(0, 0, 0, 0.25)',
];

function buildTypography() {
  return {
    fontFamily,
    h1: {
      fontFamily,
      fontSize: '2.5rem',
      fontWeight: 700,
      lineHeight: 1.2,
      letterSpacing: '-0.02em',
    },
    h2: {
      fontFamily,
      fontSize: '2rem',
      fontWeight: 700,
      lineHeight: 1.25,
      letterSpacing: '-0.01em',
    },
    h3: {
      fontFamily,
      fontSize: '1.75rem',
      fontWeight: 600,
      lineHeight: 1.3,
      letterSpacing: '-0.01em',
    },
    h4: {
      fontFamily,
      fontSize: '1.5rem',
      fontWeight: 600,
      lineHeight: 1.35,
    },
    h5: {
      fontFamily,
      fontSize: '1.25rem',
      fontWeight: 600,
      lineHeight: 1.4,
    },
    h6: {
      fontFamily,
      fontSize: '1.125rem',
      fontWeight: 600,
      lineHeight: 1.45,
    },
    subtitle1: {
      fontFamily,
      fontSize: '1rem',
      fontWeight: 500,
      lineHeight: 1.5,
    },
    subtitle2: {
      fontFamily,
      fontSize: '0.875rem',
      fontWeight: 500,
      lineHeight: 1.5,
    },
    body1: {
      fontFamily,
      fontSize: '1rem',
      fontWeight: 400,
      lineHeight: 1.6,
    },
    body2: {
      fontFamily,
      fontSize: '0.875rem',
      fontWeight: 400,
      lineHeight: 1.6,
    },
    caption: {
      fontFamily,
      fontSize: '0.75rem',
      fontWeight: 400,
      lineHeight: 1.5,
      color: paletteGrey[600],
    },
    overline: {
      fontFamily,
      fontSize: '0.75rem',
      fontWeight: 600,
      lineHeight: 1.5,
      letterSpacing: '0.08em',
      textTransform: 'uppercase' as const,
    },
    button: {
      fontFamily,
      fontSize: '0.875rem',
      fontWeight: 600,
      lineHeight: 1.5,
      textTransform: 'none' as const,
    },
  };
}

function buildLightComponents() {
  const { primary, primaryLight, primaryDark, secondary, surface } = tokens.color;

  return {
    MuiCssBaseline: {
      styleOverrides: {
        '*, *::before, *::after': {
          boxSizing: 'border-box',
          margin: 0,
          padding: 0,
        },
        ':root': {
          '--spacing-xs': tokens.space[1],
          '--spacing-sm': tokens.space[2],
          '--spacing-md': tokens.space[4],
          '--spacing-lg': tokens.space[6],
          '--spacing-xl': tokens.space[7],
          '--sidebar-width': '280px',
          '--sidebar-collapsed-width': '72px',
          '--header-height': '64px',
        },
        html: {
          WebkitFontSmoothing: 'antialiased',
          MozOsxFontSmoothing: 'grayscale',
          scrollBehavior: 'smooth',
        },
        body: {
          minHeight: '100vh',
          textRendering: 'optimizeLegibility',
          lineHeight: 1.5,
        },
        '.hover-lift': {
          transition: 'transform 0.2s ease-in-out, box-shadow 0.2s ease-in-out',
          '&:hover': {
            transform: 'translateY(-2px)',
            boxShadow: '0px 8px 24px rgba(0, 0, 0, 0.12)',
          },
        },
        '.responsive-grid-2': {
          display: 'grid',
          gap: tokens.space[6],
          gridTemplateColumns: 'repeat(2, 1fr)',
          '@media (max-width: 768px)': {
            gridTemplateColumns: '1fr',
          },
        },
        '.responsive-grid-3': {
          display: 'grid',
          gap: tokens.space[6],
          gridTemplateColumns: 'repeat(3, 1fr)',
          '@media (max-width: 1024px)': {
            gridTemplateColumns: 'repeat(2, 1fr)',
          },
          '@media (max-width: 640px)': {
            gridTemplateColumns: '1fr',
          },
        },
      },
    },

    MuiButton: {
      defaultProps: {
        disableElevation: true,
      },
      styleOverrides: {
        root: {
          borderRadius: 10,
          padding: '10px 20px',
          fontWeight: 600,
          transition: 'all 0.2s ease-in-out',
        },
        contained: {
          '&:hover': {
            transform: 'translateY(-1px)',
            boxShadow: '0px 4px 12px rgba(255, 105, 0, 0.3)',
          },
        },
        containedPrimary: {
          background: `linear-gradient(135deg, ${primary} 0%, ${primaryDark} 100%)`,
          '&:hover': {
            background: `linear-gradient(135deg, ${primaryLight} 0%, ${primary} 100%)`,
          },
        },
      },
    },

    MuiCard: {
      defaultProps: {
        elevation: 0,
      },
      styleOverrides: {
        root: {
          borderRadius: Number.parseInt(tokens.radius.lg, 10),
          border: `1px solid ${paletteGrey[200]}`,
          transition: 'all 0.3s ease-in-out',
          '&:hover': {
            borderColor: paletteGrey[300],
            boxShadow: '0px 10px 40px rgba(0, 0, 0, 0.08)',
          },
        },
      },
    },

    MuiAppBar: {
      defaultProps: {
        elevation: 0,
      },
      styleOverrides: {
        root: {
          backgroundColor: alpha(surface, 0.8),
          backdropFilter: 'blur(20px)',
          borderBottom: `1px solid ${paletteGrey[200]}`,
          color: paletteGrey[900],
          height: 'var(--header-height)',
        },
      },
    },

    MuiDrawer: {
      styleOverrides: {
        paper: {
          backgroundColor: secondary,
          color: tokens.color.onSecondary,
          borderRight: 'none',
        },
      },
    },

    MuiChip: {
      styleOverrides: {
        root: {
          borderRadius: Number.parseInt(tokens.radius.sm, 10),
          fontWeight: 500,
        },
        filled: {
          '&.MuiChip-colorSuccess': {
            backgroundColor: alpha(semanticPalette.success.main, 0.12),
            color: semanticPalette.success.dark,
          },
          '&.MuiChip-colorError': {
            backgroundColor: alpha(semanticPalette.error.main, 0.12),
            color: semanticPalette.error.dark,
          },
          '&.MuiChip-colorWarning': {
            backgroundColor: alpha(semanticPalette.warning.main, 0.12),
            color: semanticPalette.warning.dark,
          },
          '&.MuiChip-colorInfo': {
            backgroundColor: alpha(semanticPalette.info.main, 0.12),
            color: semanticPalette.info.dark,
          },
        },
      },
    },
  };
}

function buildDarkButtonComponents() {
  const { primary, primaryLight, primaryDark } = tokens.color;

  return {
    MuiButton: {
      defaultProps: { disableElevation: true },
      styleOverrides: {
        root: {
          borderRadius: 10,
          padding: '10px 20px',
          fontWeight: 600,
          transition: 'all 0.2s ease-in-out',
        },
        contained: {
          '&:hover': {
            transform: 'translateY(-1px)',
            boxShadow: '0px 4px 12px rgba(255, 105, 0, 0.4)',
          },
        },
        containedPrimary: {
          background: `linear-gradient(135deg, ${primary} 0%, ${primaryDark} 100%)`,
          '&:hover': {
            background: `linear-gradient(135deg, ${primaryLight} 0%, ${primary} 100%)`,
          },
        },
      },
    },

    MuiOutlinedInput: {
      styleOverrides: {
        root: {
          backgroundColor: alpha(tokens.darkColor.surfaceAlt, 0.6),
          '& .MuiOutlinedInput-notchedOutline': {
            borderColor: tokens.darkColor.border,
          },
          '&:hover .MuiOutlinedInput-notchedOutline': {
            borderColor: alpha(tokens.color.primary, 0.5),
          },
        },
      },
    },

    MuiToggleButton: {
      styleOverrides: {
        root: {
          color: tokens.darkColor.onSurfaceMuted,
          borderColor: tokens.darkColor.border,
          '&.Mui-selected': {
            color: tokens.color.onPrimary,
            backgroundColor: alpha(tokens.color.primary, 0.9),
            '&:hover': { backgroundColor: tokens.color.primary },
          },
        },
      },
    },
  };
}

function createLightTheme(): Theme {
  const { color } = tokens;

  return createTheme({
    palette: {
      mode: 'light',
      primary: {
        main: color.primary,
        light: color.primaryLight,
        dark: color.primaryDark,
        contrastText: color.onPrimary,
      },
      secondary: {
        main: color.secondary,
        light: color.secondaryLight,
        dark: color.secondaryDark,
        contrastText: color.onSecondary,
      },
      success: semanticPalette.success,
      warning: semanticPalette.warning,
      error: semanticPalette.error,
      info: semanticPalette.info,
      grey: paletteGrey,
      background: {
        default: color.surfaceAlt,
        paper: color.surface,
      },
      text: {
        primary: paletteGrey[900],
        secondary: paletteGrey[600],
        disabled: paletteGrey[400],
      },
      divider: paletteGrey[200],
      action: {
        active: color.primary,
        hover: alpha(color.primary, 0.08),
        selected: alpha(color.primary, 0.12),
        disabled: paletteGrey[400],
        disabledBackground: paletteGrey[200],
      },
    },
    typography: buildTypography(),
    shape: {
      borderRadius: Number.parseInt(tokens.radius.md, 10),
    },
    shadows: themeShadows,
    breakpoints: {
      values: tokens.breakpoint,
    },
    components: buildLightComponents(),
  });
}

function createDarkTheme(): Theme {
  const { color, darkColor } = tokens;
  const lightTheme = createLightTheme();

  return createTheme({
    palette: {
      mode: 'dark',
      primary: {
        main: color.primary,
        light: color.primaryLight,
        dark: color.primaryDark,
        contrastText: color.onPrimary,
      },
      secondary: {
        main: color.secondary,
        light: color.secondaryLight,
        dark: color.secondaryDark,
        contrastText: color.onSecondary,
      },
      success: semanticPalette.success,
      warning: semanticPalette.warning,
      error: semanticPalette.error,
      info: semanticPalette.info,
      grey: paletteGrey,
      background: {
        default: darkColor.background,
        paper: darkColor.surface,
      },
      text: {
        primary: darkColor.onSurface,
        secondary: darkColor.onSurfaceMuted,
        disabled: alpha(darkColor.onSurfaceMuted, 0.5),
      },
      divider: darkColor.divider,
      action: {
        active: color.primary,
        hover: alpha(color.primary, 0.12),
        selected: alpha(color.primary, 0.2),
        disabled: alpha(darkColor.onSurfaceMuted, 0.4),
        disabledBackground: alpha(darkColor.onSurfaceMuted, 0.12),
      },
    },
    typography: lightTheme.typography,
    shape: { borderRadius: Number.parseInt(tokens.radius.md, 10) },
    breakpoints: {
      values: tokens.breakpoint,
    },
    components: buildDarkButtonComponents(),
  });
}

const responsiveFontOptions = {
  breakpoints: ['sm', 'md', 'lg'] as Array<'sm' | 'md' | 'lg'>,
  factor: 2,
};

/**
 * Light admin theme built from shared design tokens (ADR-0007).
 */
export const muiTheme: Theme = responsiveFontSizes(createLightTheme(), responsiveFontOptions);

/**
 * Dark (ggCircuit-style) variant for admin login. Dashboard keeps {@link muiTheme}.
 */
export const muiDarkTheme: Theme = responsiveFontSizes(createDarkTheme(), responsiveFontOptions);

/** @deprecated Use tokens.darkColor — kept for backward-compatible imports. */
export const darkSurfaces = {
  background: tokens.darkColor.background,
  paper: tokens.darkColor.surface,
  paperAlt: tokens.darkColor.surfaceAlt,
  border: tokens.darkColor.border,
  onSurface: tokens.darkColor.onSurface,
  onSurfaceMuted: tokens.darkColor.onSurfaceMuted,
};

/** @deprecated Use tokens.color — kept for backward-compatible imports. */
export const colors = {
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
  success: semanticPalette.success,
  warning: semanticPalette.warning,
  error: semanticPalette.error,
  info: semanticPalette.info,
  grey: paletteGrey,
};
