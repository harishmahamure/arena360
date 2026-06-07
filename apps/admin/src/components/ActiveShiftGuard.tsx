import { Alert, Box, Button, CircularProgress } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { getActiveShift } from '../services/shifts';

interface ActiveShiftGuardProps {
  children: React.ReactNode;
}

export function ActiveShiftGuard({ children }: ActiveShiftGuardProps) {
  const {
    data: activeShift,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ['activeShift'],
    queryFn: getActiveShift,
    retry: false,
  });

  if (isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (isError || !activeShift) {
    return (
      <Box sx={{ px: 4, py: 3, maxWidth: 560 }}>
        <Alert severity="warning" sx={{ mb: 2 }}>
          You need an active shift before you can perform this action. Start a shift from the
          dashboard, then return here.
        </Alert>
        <Button component={Link} to="/" variant="contained">
          Go to dashboard
        </Button>
      </Box>
    );
  }

  return <>{children}</>;
}
