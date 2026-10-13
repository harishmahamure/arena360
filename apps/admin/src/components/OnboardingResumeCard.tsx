import { Alert, Button, Stack, Typography } from '@mui/material';
import { Link, useLocation } from 'react-router-dom';
import { useOnboarding } from '../hooks/useOnboarding';

export default function OnboardingResumeCard() {
  const query = useOnboarding();
  const { pathname } = useLocation();
  if (!query.canRead || pathname.startsWith('/onboarding')) return null;
  if (query.error) {
    return (
      <Alert
        severity="warning"
        action={<Button onClick={() => void query.refetch()}>Retry</Button>}
      >
        Could not load venue setup. {query.error.message}
      </Alert>
    );
  }
  if (!query.data || query.data.progress.completedAt) return null;
  const ready = query.data.readiness.checks.filter((check) => check.ready).length;
  return (
    <Alert severity="info" sx={{ mb: 2 }}>
      <Stack direction={{ xs: 'column', sm: 'row' }} gap={2} alignItems={{ sm: 'center' }}>
        <Stack sx={{ flex: 1 }}>
          <Typography fontWeight={600}>Finish setting up your venue</Typography>
          <Typography variant="body2">
            {ready} of {query.data.readiness.checks.length} readiness checks passed. Your saved step
            is {query.data.progress.currentStep}.
          </Typography>
        </Stack>
        <Button component={Link} to="/onboarding" variant="outlined">
          {query.canWrite ? 'Resume setup' : 'View setup'}
        </Button>
      </Stack>
    </Alert>
  );
}
