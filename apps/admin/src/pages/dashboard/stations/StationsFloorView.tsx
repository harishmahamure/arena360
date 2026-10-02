import {
  capRemainingByExpiry,
  DEFAULT_CAFE_TZ,
  effectiveRemainingMinutes,
  SESSION_CLOCK_TICK_MS,
} from '@gaming-cafe/contracts';
import { formatRemainingLabel } from '@gaming-cafe/utils';
import { Computer, ExpandLess, ExpandMore, SportsEsports, Tv } from '@mui/icons-material';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Collapse,
  Grid,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { type DeviceResponse, getDevices } from '../../../services/devices/list';
import { getSessions, type SessionResponse } from '../../../services/sessions/list';
import {
  deviceStatusAccentColor,
  deviceStatusColor,
  deviceStatusIcon,
  deviceStatusLabel,
} from '../../../utils/deviceStatusDisplay';

function deviceTypeIcon(deviceType: string, size: 'small' | 'medium' | 'large' = 'small') {
  const type = deviceType.toUpperCase();
  if (type === 'PC') return <Computer fontSize={size} />;
  if (type === 'PS5' || type === 'PS4' || type === 'CONSOLE') {
    return <SportsEsports fontSize={size} />;
  }
  return <Tv fontSize={size} />;
}

function sortByName(a: DeviceResponse, b: DeviceResponse): number {
  return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
}

function countAvailableByType(devices: DeviceResponse[]): { type: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const d of devices) {
    const key = d.deviceType.toUpperCase();
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([type, count]) => ({ type, count }))
    .sort((a, b) => a.type.localeCompare(b.type));
}

function deviceTypeCaption(device: DeviceResponse): string {
  const parts = [device.deviceType];
  if (device.deviceSubType) parts.push(device.deviceSubType);
  else if (device.location) parts.push(device.location);
  return parts.join(' · ');
}

function sessionRemainingMinutes(session: SessionResponse): number | null {
  if (!session.startTime || !session.balance) return null;
  return capRemainingByExpiry(
    effectiveRemainingMinutes(
      session.startTime,
      session.balance.remainingMinutes,
      session.timeCreditsConsumed ?? 0,
      session.balance.deductionProfile,
      session.cafeTimezone ?? DEFAULT_CAFE_TZ,
    ),
    session.balance.expiryDate,
  );
}

export function StationsFloorView() {
  const { data, isLoading: devicesLoading } = useQuery({
    queryKey: ['devices', 'stations-floor'],
    queryFn: () =>
      getDevices({
        limit: 100,
        sortBy: 'name',
        sortOrder: 'ASC',
      }),
    refetchInterval: 30_000,
  });

  const { data: activeSessionsData, isLoading: sessionsLoading } = useQuery({
    queryKey: ['sessions', 'stations-floor-active'],
    queryFn: () => getSessions({ isActive: 1, limit: 100 }),
    refetchInterval: 30_000,
  });

  const [clockTick, setClockTick] = useState(0);
  const [otherExpanded, setOtherExpanded] = useState(false);

  useEffect(() => {
    const id = setInterval(() => setClockTick((t) => t + 1), SESSION_CLOCK_TICK_MS);
    return () => clearInterval(id);
  }, []);

  const sessionByDeviceId = useMemo(() => {
    const map = new Map<string, SessionResponse>();
    for (const session of activeSessionsData?.data ?? []) {
      if (!session.endTime) {
        map.set(session.deviceId, session);
      }
    }
    return map;
  }, [activeSessionsData?.data]);

  const { inUseDevices, availableDevices, otherDevices } = useMemo(() => {
    void clockTick;
    const list = data?.data ?? [];
    const inUse = list.filter((d) => d.status === 'in_use').sort(sortByName);
    const available = list
      .filter((d) => d.status === 'available' || d.status === 'operational')
      .sort(sortByName);
    const other = list
      .filter((d) => d.status === 'under_maintenance' || d.status === 'out_of_service')
      .sort(sortByName);
    return { inUseDevices: inUse, availableDevices: available, otherDevices: other };
  }, [data?.data, clockTick]);

  const inUseCount = inUseDevices.length;
  const availableCount = availableDevices.length;
  const otherCount = otherDevices.length;
  const availableByType = useMemo(() => countAvailableByType(availableDevices), [availableDevices]);
  const totalCount = inUseCount + availableCount + otherCount;
  const isLoading = devicesLoading || sessionsLoading;

  return (
    <Stack spacing={3}>
      <Stack
        direction="row"
        alignItems="center"
        justifyContent="space-between"
        gap={2}
        flexWrap="wrap"
      >
        <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: 'wrap' }}>
          <Chip size="medium" color="primary" variant="outlined" label={`${inUseCount} in use`} />
          <Chip size="medium" color="success" variant="outlined" label={`${availableCount} free`} />
        </Stack>
        <Button component={RouterLink} to="/devices" size="small" variant="outlined">
          Manage devices
        </Button>
      </Stack>

      {isLoading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
          <CircularProgress size={36} />
        </Box>
      ) : totalCount === 0 ? (
        <Typography variant="body1" color="text.secondary" sx={{ py: 4 }}>
          No stations configured yet. Add devices to see the floor view.
        </Typography>
      ) : (
        <Stack spacing={4}>
          {inUseDevices.length > 0 ? (
            <Stack spacing={2}>
              <Typography variant="subtitle1" fontWeight={600}>
                In use
              </Typography>
              <Grid container spacing={2}>
                {inUseDevices.map((device) => (
                  <Grid key={device.id} size={{ xs: 12, md: 6, lg: 4 }}>
                    <DeviceStationRow device={device} session={sessionByDeviceId.get(device.id)} />
                  </Grid>
                ))}
              </Grid>
            </Stack>
          ) : null}

          <Stack spacing={2}>
            <Stack
              direction="row"
              alignItems="center"
              justifyContent="space-between"
              gap={2}
              flexWrap="wrap"
            >
              <Typography variant="subtitle1" fontWeight={600}>
                Available
              </Typography>
              {availableByType.length > 0 ? (
                <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: 'wrap' }}>
                  {availableByType.map(({ type, count }) => (
                    <Chip
                      key={type}
                      size="small"
                      color="success"
                      variant="outlined"
                      label={`${type} · ${count}`}
                    />
                  ))}
                </Stack>
              ) : null}
            </Stack>

            {availableCount === 0 ? (
              <Typography variant="body2" color="text.secondary">
                No stations free
              </Typography>
            ) : (
              <Grid container spacing={2}>
                {availableDevices.map((device) => (
                  <Grid key={device.id} size={{ xs: 6, sm: 4, md: 3, lg: 2 }}>
                    <AvailableStationTile device={device} />
                  </Grid>
                ))}
              </Grid>
            )}
          </Stack>

          {otherCount > 0 ? (
            <Stack spacing={2}>
              <Button
                color="inherit"
                onClick={() => setOtherExpanded((open) => !open)}
                endIcon={otherExpanded ? <ExpandLess /> : <ExpandMore />}
                sx={{
                  justifyContent: 'flex-start',
                  px: 0,
                  minHeight: 40,
                  color: 'text.secondary',
                  fontWeight: 600,
                  fontSize: '0.9375rem',
                  textTransform: 'none',
                  alignSelf: 'flex-start',
                }}
              >
                Other ({otherCount})
              </Button>
              <Collapse in={otherExpanded}>
                <Grid container spacing={2}>
                  {otherDevices.map((device) => (
                    <Grid key={device.id} size={{ xs: 12, md: 6, lg: 4 }}>
                      <DeviceStationRow
                        device={device}
                        session={sessionByDeviceId.get(device.id)}
                      />
                    </Grid>
                  ))}
                </Grid>
              </Collapse>
            </Stack>
          ) : null}
        </Stack>
      )}
    </Stack>
  );
}

function AvailableStationTile({ device }: { device: DeviceResponse }) {
  return (
    <Box
      component={RouterLink}
      to={`/devices/${device.id}`}
      sx={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        width: '100%',
        minHeight: 148,
        textDecoration: 'none',
        color: 'inherit',
        borderRadius: 2,
        border: 1,
        borderColor: 'divider',
        borderLeftWidth: 4,
        borderLeftColor: 'success.main',
        px: 2,
        py: 2.5,
        transition: 'background-color 0.15s ease, box-shadow 0.15s ease',
        '&:hover': {
          bgcolor: 'action.hover',
          boxShadow: 1,
        },
      }}
    >
      <Box sx={{ color: 'text.secondary', display: 'flex', mb: 1.5 }}>
        {deviceTypeIcon(device.deviceType, 'large')}
      </Box>
      <Typography
        variant="h6"
        fontWeight={600}
        noWrap
        sx={{ width: '100%', textAlign: 'center', lineHeight: 1.25, fontSize: '1.05rem' }}
      >
        {device.name}
      </Typography>
      <Typography
        variant="body2"
        color="text.secondary"
        noWrap
        sx={{ width: '100%', textAlign: 'center', mt: 0.75 }}
      >
        {deviceTypeCaption(device)}
      </Typography>
    </Box>
  );
}

function DeviceStationRow({
  device,
  session,
}: {
  device: DeviceResponse;
  session?: SessionResponse;
}) {
  const isActive = device.status === 'in_use' && session;
  const player = session?.balance?.player?.username;
  const remaining = session ? sessionRemainingMinutes(session) : null;
  const href = isActive && session ? `/sessions/${session.id}` : `/devices/${device.id}`;

  return (
    <Box
      component={RouterLink}
      to={href}
      sx={{
        display: 'block',
        height: '100%',
        textDecoration: 'none',
        color: 'inherit',
        borderRadius: 1.5,
        border: 1,
        borderColor: 'divider',
        borderLeftWidth: 4,
        borderLeftColor: deviceStatusAccentColor(device.status),
        px: 2,
        py: 1.5,
        transition: 'background-color 0.15s ease',
        '&:hover': {
          bgcolor: 'action.hover',
        },
      }}
    >
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" gap={1}>
        <Stack direction="row" spacing={1.5} alignItems="center" sx={{ minWidth: 0, flex: 1 }}>
          <Box sx={{ color: 'text.secondary', display: 'flex', flexShrink: 0 }}>
            {deviceTypeIcon(device.deviceType, 'medium')}
          </Box>
          <Box sx={{ minWidth: 0 }}>
            <Typography variant="body1" fontWeight={600} noWrap>
              {device.name}
            </Typography>
            <Typography variant="body2" color="text.secondary" noWrap display="block">
              {device.deviceType}
              {device.location ? ` · ${device.location}` : ''}
            </Typography>
          </Box>
        </Stack>
        <Chip
          size="small"
          icon={deviceStatusIcon(device.status) ?? undefined}
          label={deviceStatusLabel(device.status)}
          color={deviceStatusColor(device.status)}
          variant={device.status === 'in_use' ? 'filled' : 'outlined'}
          sx={{ flexShrink: 0 }}
        />
      </Stack>

      {isActive && player ? (
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1, display: 'block' }}>
          @{player}
          {remaining != null ? ` · ${formatRemainingLabel(remaining)} left` : ''}
        </Typography>
      ) : null}
    </Box>
  );
}
