'use client';

import { alpha, createTheme, responsiveFontSizes } from '@mui/material/styles';

export const colors = {
  primary: {
    main: '#FF6900',
    light: '#FF8A3D',
    dark: '#CC5400',
    contrastText: '#FFFFFF',
  },
  secondary: {
    main: '#1A1A2E',
    light: '#2D2D44',
    dark: '#0F0F1A',
    contrastText: '#FFFFFF',
  },
  accent: {
    coral: '#FF6B6B',
    teal: '#4ECDC4',
    gold: '#FFD93D',
    purple: '#6C5CE7',
  },
  success: {
    main: '#10B981',
    light: '#34D399',
    dark: '#059669',
    contrastText: '#FFFFFF',
  },
  warning: {
    main: '#F59E0B',
    light: '#FBBF24',
    dark: '#D97706',
    contrastText: '#1A1A2E',
  },
  error: {
    main: '#EF4444',
    light: '#F87171',
    dark: '#DC2626',
    contrastText: '#FFFFFF',
  },
  info: {
    main: '#3B82F6',
    light: '#60A5FA',
    dark: '#2563EB',
    contrastText: '#FFFFFF',
  },
  grey: {
    50: '#FAFAFA',
    100: '#F4F4F5',
    200: '#E4E4E7',
    300: '#D4D4D8',
    400: '#A1A1AA',
    500: '#71717A',
    600: '#52525B',
    700: '#3F3F46',
    800: '#27272A',
    900: '#18181B',
  },
};

const fontFamily = '"Zalando Sans", Roboto, Arial, sans-serif';

const theme = createTheme({
  palette: {
    mode: 'light',
    primary: colors.primary,
    secondary: colors.secondary,
    success: colors.success,
    warning: colors.warning,
    error: colors.error,
    info: colors.info,
    grey: colors.grey,
    background: {
      default: '#F8F9FC',
      paper: '#FFFFFF',
    },
    text: {
      primary: colors.grey[900],
      secondary: colors.grey[600],
      disabled: colors.grey[400],
    },
    divider: colors.grey[200],
    action: {
      active: colors.primary.main,
      hover: alpha(colors.primary.main, 0.08),
      selected: alpha(colors.primary.main, 0.12),
      disabled: colors.grey[400],
      disabledBackground: colors.grey[200],
    },
  },

  typography: {
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
      color: colors.grey[600],
    },
    overline: {
      fontFamily,
      fontSize: '0.75rem',
      fontWeight: 600,
      lineHeight: 1.5,
      letterSpacing: '0.08em',
      textTransform: 'uppercase',
    },
    button: {
      fontFamily,
      fontSize: '0.875rem',
      fontWeight: 600,
      lineHeight: 1.5,
      textTransform: 'none',
    },
  },

  shape: {
    borderRadius: 12,
  },

  shadows: [
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
  ],

  breakpoints: {
    values: {
      xs: 0,
      sm: 640,
      md: 768,
      lg: 1024,
      xl: 1280,
    },
  },

  components: {
    MuiCssBaseline: {
      styleOverrides: {
        '*, *::before, *::after': {
          boxSizing: 'border-box',
          margin: 0,
          padding: 0,
        },
        ':root': {
          '--spacing-xs': '4px',
          '--spacing-sm': '8px',
          '--spacing-md': '16px',
          '--spacing-lg': '24px',
          '--spacing-xl': '32px',
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
          gap: '24px',
          gridTemplateColumns: 'repeat(2, 1fr)',
          '@media (max-width: 768px)': {
            gridTemplateColumns: '1fr',
          },
        },
        '.responsive-grid-3': {
          display: 'grid',
          gap: '24px',
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
          background: `linear-gradient(135deg, ${colors.primary.main} 0%, ${colors.primary.dark} 100%)`,
          '&:hover': {
            background: `linear-gradient(135deg, ${colors.primary.light} 0%, ${colors.primary.main} 100%)`,
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
          borderRadius: 16,
          border: `1px solid ${colors.grey[200]}`,
          transition: 'all 0.3s ease-in-out',
          '&:hover': {
            borderColor: colors.grey[300],
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
          backgroundColor: alpha('#FFFFFF', 0.8),
          backdropFilter: 'blur(20px)',
          borderBottom: `1px solid ${colors.grey[200]}`,
          color: colors.grey[900],
          height: 'var(--header-height)',
        },
      },
    },

    MuiDrawer: {
      styleOverrides: {
        paper: {
          backgroundColor: colors.secondary.main,
          color: '#FFFFFF',
          borderRight: 'none',
        },
      },
    },

    MuiChip: {
      styleOverrides: {
        root: {
          borderRadius: 8,
          fontWeight: 500,
        },
        filled: {
          '&.MuiChip-colorSuccess': {
            backgroundColor: alpha(colors.success.main, 0.12),
            color: colors.success.dark,
          },
          '&.MuiChip-colorError': {
            backgroundColor: alpha(colors.error.main, 0.12),
            color: colors.error.dark,
          },
          '&.MuiChip-colorWarning': {
            backgroundColor: alpha(colors.warning.main, 0.12),
            color: colors.warning.dark,
          },
          '&.MuiChip-colorInfo': {
            backgroundColor: alpha(colors.info.main, 0.12),
            color: colors.info.dark,
          },
        },
      },
    },
  },
});

const responsiveTheme = responsiveFontSizes(theme, {
  breakpoints: ['sm', 'md', 'lg'],
  factor: 2,
});

export default responsiveTheme;
