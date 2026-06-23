import {
  capRemainingByExpiry,
  DEFAULT_CAFE_TZ,
  effectiveRemainingMinutes,
  SESSION_CLOCK_TICK_MS,
} from '@gaming-cafe/contracts';
import { EmptyState, ErrorPanel, PageShell } from '@gaming-cafe/ui';
import { formatRemainingLabel, toastUtils } from '@gaming-cafe/utils';
import {
  AccessTime,
  Devices,
  Login,
  Logout,
  People,
  PlayCircle,
  PointOfSale,
  Receipt,
  ShoppingCart,
} from '@mui/icons-material';
import { Box, Button, Card, CardContent, Chip, Grid, Skeleton, Typography } from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { StatCard, type StatTone } from '../../containers/stats/StatCard';
import { useStaffDashboardStats } from '../../hooks/useStaffDashboardStats';
import type { SessionResponse } from '../../services/sessions/list';
import { getSessions } from '../../services/sessions/list';
import { clockIn } from '../../services/shifts';
import { formatTypePaymentSubtitle, normalizeRevenue } from '../../services/stats/statsHelpers';
import { formatDisplayDateTime, formatDuration, now as nowDate } from '../../utils/date';

const ENDING_SOON_MINUTES = 15;
const ENDING_SOON_MAX = 5;

function sessionEffectiveRemainingMinutes(session: SessionResponse): number | null {
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

const QUICK_ACTION_SX = {
  minHeight: 44,
  width: '100%',
} as const;

const quickActions = [
  {
    label: 'Start session',
    path: '/sessions/new',
    icon: PlayCircle,
    variant: 'contained' as const,
  },
  {
    label: 'Sell items',
    path: '/product-transactions/new',
    icon: PointOfSale,
    variant: 'outlined' as const,
  },
  {
    label: 'Buy plan',
    path: '/plan-transactions/new',
    icon: Receipt,
    variant: 'outlined' as const,
  },
  {
    label: 'Active sessions',
    path: '/sessions?active=true',
    icon: AccessTime,
    variant: 'outlined' as const,
  },
];

function StaffDashboardSkeleton() {
  return (
    <PageShell>
      <Skeleton variant="rounded" height={88} sx={{ mb: 3 }} />
      <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', mb: 3 }}>
        {['qa-1', 'qa-2', 'qa-3', 'qa-4'].map((id) => (
          <Skeleton key={id} variant="rounded" width={140} height={44} />
        ))}
      </Box>
      <Skeleton variant="text" width={200} height={28} sx={{ mb: 2 }} />
      <Grid container spacing={3}>
        {['stat-1', 'stat-2', 'stat-3', 'stat-4', 'stat-5', 'stat-6'].map((id) => (
          <Grid key={id} size={{ xs: 12, sm: 6, md: 4 }}>
            <Skeleton variant="rounded" height={160} />
          </Grid>
        ))}
      </Grid>
    </PageShell>
  );
}

export default function StaffDashboardView() {
  const queryClient = useQueryClient();
  const [startingShift, setStartingShift] = useState(false);
  const { data: stats, isLoading, error, refetch } = useStaffDashboardStats();

  const shiftActive = !!stats?.shift;

  const { data: activeSessionsData } = useQuery({
    queryKey: ['sessions', 'dashboard-active'],
    queryFn: () => getSessions({ isActive: 1, limit: 50 }),
    enabled: shiftActive,
  });

  const enrichedSessions = activeSessionsData?.data ?? [];

  // Local tick so "ending soon" decays without server refetch (matches SessionRemainingClock).
  const [clockTick, setClockTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setClockTick((t) => t + 1), SESSION_CLOCK_TICK_MS);
    return () => clearInterval(id);
  }, []);

  const endingSoonSessions = useMemo(() => {
    void clockTick;
    return enrichedSessions
      .filter((session) => {
        if (session.endTime) return false;
        const remaining = sessionEffectiveRemainingMinutes(session);
        return remaining !== null && remaining <= ENDING_SOON_MINUTES;
      })
      .sort((a, b) => {
        const aRem = sessionEffectiveRemainingMinutes(a) ?? 0;
        const bRem = sessionEffectiveRemainingMinutes(b) ?? 0;
        return aRem - bRem;
      })
      .slice(0, ENDING_SOON_MAX);
  }, [enrichedSessions, clockTick]);

  const handleStartShift = async () => {
    setStartingShift(true);
    try {
      await clockIn();
      toastUtils.success('Shift started');
      void queryClient.invalidateQueries({ queryKey: ['activeShift'] });
      void queryClient.invalidateQueries({ queryKey: ['staffDashboardStats'] });
      void queryClient.invalidateQueries({ queryKey: ['shifts'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to start shift');
    } finally {
      setStartingShift(false);
    }
  };

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const formatPaymentBreakdown = (revenue?: {
    cashRevenue?: number;
    onlineRevenue?: number;
    creditRevenue?: number;
  }) =>
    `Cash: ${formatCurrency(revenue?.cashRevenue ?? 0)} | Online: ${formatCurrency(revenue?.onlineRevenue ?? 0)} | Credit: ${formatCurrency(revenue?.creditRevenue ?? 0)}`;

  const shiftDuration = useMemo(() => {
    if (!stats?.shift?.startDate) return null;
    const start = new Date(stats.shift.startDate);
    const diffMs = nowDate().getTime() - start.getTime();
    return formatDuration(diffMs / 60000);
  }, [stats?.shift?.startDate]);

  if (isLoading) {
    return <StaffDashboardSkeleton />;
  }

  if (error || !stats) {
    return (
      <PageShell>
        <ErrorPanel
          message="Failed to load dashboard statistics. Please try again later."
          onRetry={() => void refetch()}
        />
      </PageShell>
    );
  }

  const shiftRevenue = normalizeRevenue(stats.shiftRevenue ?? undefined);

  const collectionCards: Array<{
    title: string;
    value: string;
    subtitle: string;
    icon: typeof PointOfSale;
    tone: StatTone;
  }> = [
    {
      title: 'Collections This Shift',
      value: formatCurrency(shiftRevenue.total),
      subtitle: formatPaymentBreakdown(shiftRevenue),
      icon: PointOfSale,
      tone: 'success',
    },
    {
      title: 'Plan Transactions',
      value: formatCurrency(shiftRevenue.plan),
      subtitle: formatTypePaymentSubtitle(shiftRevenue, 'plan', formatCurrency),
      icon: Receipt,
      tone: 'primary',
    },
    {
      title: 'Product Transactions',
      value: formatCurrency(shiftRevenue.merchandise),
      subtitle: formatTypePaymentSubtitle(shiftRevenue, 'product', formatCurrency),
      icon: ShoppingCart,
      tone: 'info',
    },
    {
      title: 'Active Sessions',
      value: stats.sessions.activeSessions.toLocaleString(),
      subtitle: `Completed: ${stats.sessions.completedSessions} | Avg: ${stats.sessions.averageSessionDuration.toFixed(1)} min`,
      icon: AccessTime,
      tone: 'warning',
    },
    {
      title: 'Active Players',
      value: stats.players.activePlayers.toLocaleString(),
      subtitle: `New this shift: ${stats.players.newPlayersInPeriod}`,
      icon: People,
      tone: 'primary',
    },
    {
      title: 'Devices',
      value: `${stats.devices.available} available`,
      subtitle: `In use: ${stats.devices.inUse} | Total: ${stats.devices.total}`,
      icon: Devices,
      tone: 'error',
    },
  ];

  return (
    <PageShell>
      <Card
        variant="outlined"
        sx={{
          mb: 3,
          bgcolor: stats.shift ? 'success.50' : 'warning.50',
          borderColor: stats.shift ? 'success.main' : 'warning.main',
        }}
      >
        <CardContent
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            gap: 2,
            py: 2,
            '&:last-child': { pb: 2 },
          }}
        >
          <Box sx={{ display: 'flex', alignItems: 'flex-start', gap: 2, flex: 1 }}>
            {stats.shift ? (
              <Login sx={{ color: 'success.main', mt: 0.5 }} />
            ) : (
              <Logout sx={{ color: 'warning.main', mt: 0.5 }} />
            )}
            <Box>
              <Typography variant="subtitle1" fontWeight={600}>
                {stats.shift ? 'Shift active' : 'No active shift'}
              </Typography>
              {stats.shift ? (
                <>
                  <Typography variant="h4" fontWeight={700} color="success.main" sx={{ mt: 0.5 }}>
                    {formatCurrency(shiftRevenue.total)}
                  </Typography>
                  <Typography variant="caption" color="text.secondary" display="block">
                    Collected this shift
                  </Typography>
                  <Typography
                    variant="caption"
                    color="text.secondary"
                    display="block"
                    sx={{ mt: 0.5 }}
                  >
                    {formatPaymentBreakdown(shiftRevenue)}
                  </Typography>
                  <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                    Started {formatDisplayDateTime(stats.shift.startDate)}
                    {shiftDuration && ` \u2022 Duration: ${shiftDuration}`}
                  </Typography>
                </>
              ) : (
                <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                  Start a shift to track collections and use counter actions.
                </Typography>
              )}
            </Box>
          </Box>
          {!stats.shift && (
            <Button
              variant="contained"
              onClick={() => void handleStartShift()}
              disabled={startingShift}
              sx={{ minHeight: 44 }}
            >
              {startingShift ? 'Starting…' : 'Start shift'}
            </Button>
          )}
        </CardContent>
      </Card>

      {stats.shift && (
        <Box sx={{ mb: 3 }}>
          <Typography variant="subtitle2" color="text.secondary" sx={{ mb: 1.5 }}>
            Quick actions
          </Typography>
          <Grid container spacing={1.5}>
            {quickActions.map((action) => {
              const Icon = action.icon;
              return (
                <Grid key={action.path} size={{ xs: 6, sm: 'auto' }}>
                  <Button
                    component={RouterLink}
                    to={action.path}
                    variant={action.variant}
                    startIcon={<Icon />}
                    sx={QUICK_ACTION_SX}
                  >
                    {action.label}
                  </Button>
                </Grid>
              );
            })}
          </Grid>
        </Box>
      )}

      {stats.shift && endingSoonSessions.length > 0 && (
        <Box sx={{ mb: 3 }}>
          <Typography variant="subtitle2" color="text.secondary" sx={{ mb: 1 }}>
            Sessions ending soon
          </Typography>
          <Box
            sx={{
              display: 'flex',
              gap: 1,
              overflowX: 'auto',
              pb: 0.5,
              flexWrap: 'nowrap',
            }}
          >
            {endingSoonSessions.map((session) => {
              const player = session.balance?.player?.username ?? 'Unknown';
              const device = session.device?.name ?? 'Unknown';
              const remainingLabel = formatRemainingLabel(
                sessionEffectiveRemainingMinutes(session),
              );
              return (
                <Chip
                  key={session.id}
                  component={RouterLink}
                  to={`/sessions/${session.id}`}
                  clickable
                  color="warning"
                  variant="outlined"
                  label={`${player} · ${device} · ${remainingLabel} left`}
                  sx={{ flexShrink: 0 }}
                />
              );
            })}
          </Box>
        </Box>
      )}

      <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
        Shift collection summary
      </Typography>

      {stats.shift ? (
        <Grid container spacing={3}>
          {collectionCards.map((card) => {
            const Icon = card.icon;
            return (
              <Grid key={card.title} size={{ xs: 12, sm: 6, md: 4 }}>
                <StatCard
                  title={card.title}
                  value={card.value}
                  subtitle={card.subtitle}
                  icon={<Icon />}
                  tone={card.tone}
                />
              </Grid>
            );
          })}
        </Grid>
      ) : (
        <EmptyState
          title="No shift collections yet"
          description="Start your shift to track sales and sessions for this period."
          actionLabel="Start shift"
          onAction={() => void handleStartShift()}
        />
      )}
    </PageShell>
  );
}
