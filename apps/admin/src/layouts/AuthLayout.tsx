import { darkTheme } from '@gaming-cafe/theme';
import { AuthLayout as BaseAuthLayout } from '@gaming-cafe/ui';
import CssBaseline from '@mui/material/CssBaseline';
import { ThemeProvider } from '@mui/material/styles';
import { Outlet } from 'react-router-dom';

/**
 * Admin login shell. Renders the shared video-background glass card under a
 * local dark theme so only the auth route is dark; the dashboard keeps the
 * light theme from `Providers`.
 */
export default function AuthLayout() {
  return (
    <ThemeProvider theme={darkTheme}>
      <CssBaseline />
      <BaseAuthLayout>
        <Outlet />
      </BaseAuthLayout>
    </ThemeProvider>
  );
}
