import { local } from '@gaming-cafe/utils';
import { Box, Container, LinearProgress, Typography } from '@mui/material';
import { Suspense } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import { BrandMark } from '../components/BrandMark';
import { useSelector } from '../hooks/store';

export default function OnboardingLayout() {
  const role = useSelector((state) => state.auth.role);
  if (!local.get('accessToken') || !role) return <Navigate to="/login" replace />;
  return (
    <Box sx={{ minHeight: '100dvh', bgcolor: 'background.default' }}>
      <Box
        component="header"
        sx={{ bgcolor: 'background.paper', borderBottom: 1, borderColor: 'divider', py: 2 }}
      >
        <Container
          maxWidth="md"
          sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 2 }}
        >
          <Box className="workspace-brand" sx={{ color: 'text.primary', m: 0, p: 0 }}>
            <BrandMark />
          </Box>
          <Typography variant="body2" color="text.secondary">
            Venue setup
          </Typography>
        </Container>
      </Box>
      <Container component="main" maxWidth="md" sx={{ py: { xs: 3, sm: 5 } }}>
        <Suspense fallback={<LinearProgress aria-label="Loading venue setup" />}>
          <Outlet />
        </Suspense>
      </Container>
    </Box>
  );
}
