import { Permission } from '@gaming-cafe/contracts';
import { local } from '@gaming-cafe/utils';
import { Alert, Button, Container, LinearProgress, Stack } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { type ReactNode, useEffect } from 'react';
import { Link, Navigate, useLocation } from 'react-router-dom';
import { useOnboarding } from '../hooks/useOnboarding';
import { usePermissions } from '../hooks/usePermissions';
import { clearAdminSession, panelClaims } from '../lib/authSession';
import { selectedLocationId, selectLocation } from '../lib/locationSelection';
import { getVenueLocations } from '../services/config';
import { getShiftStartContext } from '../services/shifts';
import LocationChoice from './LocationChoice';

// Organization setup and account pages remain available when no venue is configured.
const managementPaths = ['/locations', '/access', '/settings', '/profile'];

export default function OperatingLocationGate({ children }: { children: ReactNode }) {
  const { can, isStaff } = usePermissions();
  const path = useLocation().pathname;
  const setup = useOnboarding();
  const organizationId = panelClaims()?.tenantId ?? '';
  const authenticated = Boolean(local.get('accessToken'));
  const management = managementPaths.some(
    (prefix) => path === prefix || path.startsWith(`${prefix}/`),
  );
  const checkingSetup = path === '/' && setup.canWrite && setup.isPending;
  const freshSetup =
    path === '/' &&
    setup.canWrite &&
    setup.data &&
    !setup.data.progress.startedAt &&
    !setup.data.progress.completedAt;
  const enabled =
    authenticated &&
    !!organizationId &&
    can(Permission.LocationsRead) &&
    !management &&
    !checkingSetup &&
    !freshSetup;
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
    enabled,
    retry: false,
  });
  const available = locations.data?.filter((location) => location.isActive) ?? [];
  const saved = selectedLocationId();
  const selected = available.find((location) => location.id === saved);
  const single = available.length === 1 ? available[0] : undefined;
  useEffect(() => {
    if (enabled && !locations.isFetching && !locations.isError && !selected && single)
      selectLocation(single.id);
  }, [enabled, locations.isFetching, locations.isError, selected, single]);
  // An existing shift must belong to the chosen venue before any operational data loads.
  const shift = useQuery({
    queryKey: ['shift-start-context', selected?.id],
    queryFn: () => getShiftStartContext(selected?.id),
    enabled: enabled && !!selected && isStaff,
    retry: false,
  });
  if (!authenticated) return <Navigate to="/login" replace />;
  if (!organizationId && can(Permission.LocationsRead))
    return (
      <Alert severity="error" action={<Button onClick={clearAdminSession}>Sign out</Button>}>
        Your account has no organization. Sign in again before continuing.
      </Alert>
    );
  if (checkingSetup) return <LinearProgress aria-label="Checking venue setup" />;
  if (freshSetup) return <Navigate to="/onboarding" replace />;
  if (management || (!can(Permission.LocationsRead) && !isStaff)) return children;
  if (!can(Permission.LocationsRead))
    return (
      <Alert severity="error">
        Location access is required before counter operations. Ask an administrator to assign
        access.
      </Alert>
    );
  if (locations.isError)
    return (
      <Alert
        severity="error"
        action={<Button onClick={() => void locations.refetch()}>Retry</Button>}
      >
        Locations could not be loaded. {locations.error.message}
      </Alert>
    );
  if (locations.isPending || (single && !selected))
    return <LinearProgress aria-label="Preparing your location" />;
  if (!available.length)
    return (
      <Container maxWidth="sm" sx={{ py: 5 }}>
        <Alert severity="warning">
          No active location is available. Ask an administrator to configure a location or assign
          you access.
        </Alert>
        {can(Permission.LocationsManage) && (
          <Button component={Link} to="/locations" sx={{ mt: 2 }}>
            Manage locations
          </Button>
        )}
        {setup.canWrite && (
          <Button component={Link} to="/onboarding" sx={{ mt: 2 }}>
            Resume venue setup
          </Button>
        )}
        <Button onClick={clearAdminSession} sx={{ mt: 2 }}>
          Sign out
        </Button>
      </Container>
    );
  if (selected && isStaff && shift.isPending)
    return <LinearProgress aria-label="Checking your shift location" />;
  if (!selected || shift.isError)
    return (
      <Container maxWidth="sm">
        <Stack gap={2}>
          {shift.isError && (
            <Alert
              severity="error"
              action={<Button onClick={() => void shift.refetch()}>Retry</Button>}
            >
              {shift.error.message} Retry or choose another location before continuing.
            </Alert>
          )}
          {available.length > 1 && (
            <LocationChoice
              locations={available}
              onChoose={selectLocation}
              action={path === '/' ? 'Open dashboard' : 'Continue to workspace'}
            />
          )}
          {setup.canWrite && setup.data && !setup.data.progress.completedAt && (
            <Button component={Link} to="/onboarding">
              Resume venue setup
            </Button>
          )}
          <Button onClick={clearAdminSession}>Sign out</Button>
        </Stack>
      </Container>
    );
  return children;
}
