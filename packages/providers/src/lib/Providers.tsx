import { theme } from '@gaming-cafe/theme';
import { TOAST_CONTAINER_PROPS } from '@gaming-cafe/utils';
import CssBaseline from '@mui/material/CssBaseline';
import { ThemeProvider } from '@mui/material/styles';
import { ToastContainer } from 'react-toastify';
import 'react-toastify/dist/ReactToastify.css';
import '../toast-overrides.css';

interface ProvidersProps {
  children: React.ReactNode;
}

export function Providers({ children }: ProvidersProps) {
  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      {children}
      <ToastContainer {...TOAST_CONTAINER_PROPS} theme="light" />
    </ThemeProvider>
  );
}
