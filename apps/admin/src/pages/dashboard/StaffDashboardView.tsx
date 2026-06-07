import { toastUtils } from '@gaming-cafe/utils';
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
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  Divider,
  GridLegacy as Grid,
  Stack,
  Typography,
} from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { useStaffDashboardStats } from '../../hooks/useStaffDashboardStats';
import { clockIn } from '../../services/shifts';
import { formatDisplayDateTime, formatDuration, now as nowDate } from '../../utils/date';

const quickActions = [
  { label: 'Start session', path: '/sessions/new', icon: PlayCircle, variant: 'contained' as const },
  { label: 'Sell items', path: '/product-transactions/new', icon: PointOfSale, variant: 'outlined' as const },
  { label: 'Buy plan', path: '/plan-transactions/new', icon: Receipt, variant: 'outlined' as const },
  { label: 'Active sessions', path: '/sessions?active=true', icon: AccessTime, variant: 'outlined' as const },
];

export default function StaffDashboardView() {
  const queryClient = useQueryClient();
  const [startingShift, setStartingShift] = useState(false);
  const { data: stats, isLoading, error } = useStaffDashboardStats();

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

  const shiftDuration = useMemo(() => {
    if (!stats?.shift?.startDate) return null;
    const start = new Date(stats.shift.startDate);
    const diffMs = nowDate().getTime() - start.getTime();
    return formatDuration(diffMs / 60000);
  }, [stats?.shift?.startDate]);

  if (isLoading) {
    return (
      <Box
        sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '80vh' }}
      >
        <CircularProgress />
      </Box>
    );
  }

  if (error || !stats) {
    return (
      <Box sx={{ py: 4, px: 3 }}>
        <Alert severity="error">Failed to load dashboard statistics. Please try again later.</Alert>
      </Box>
    );
  }

  const collectionCards = [
    {
      title: 'Collections This Shift',
      value: formatCurrency(stats.shiftRevenue?.total ?? 0),
      subtitle: `Cash: ${formatCurrency(stats.shiftRevenue?.cashRevenue ?? 0)} | Online: ${formatCurrency(stats.shiftRevenue?.onlineRevenue ?? 0)}`,
      icon: PointOfSale,
      color: '#059669',
    },
    {
      title: 'Transactions',
      value: stats.transactions.totalTransactions.toLocaleString(),
      subtitle: `Completed: ${stats.transactions.completedTransactions} | Pending: ${stats.transactions.pendingTransactions}`,
      icon: ShoppingCart,
      color: '#3B82F6',
    },
    {
      title: 'Active Sessions',
      value: stats.sessions.activeSessions.toLocaleString(),
      subtitle: `Completed: ${stats.sessions.completedSessions} | Avg: ${stats.sessions.averageSessionDuration.toFixed(1)} min`,
      icon: AccessTime,
      color: '#F59E0B',
    },
    {
      title: 'Active Players',
      value: stats.players.activePlayers.toLocaleString(),
      subtitle: `New this shift: ${stats.players.newPlayersInPeriod}`,
      icon: People,
      color: '#8B5CF6',
    },
    {
      title: 'Devices',
      value: `${stats.devices.available} available`,
      subtitle: `In use: ${stats.devices.inUse} | Total: ${stats.devices.total}`,
      icon: Devices,
      color: '#EF4444',
    },
  ];

  return (
    <Box sx={{ py: { xs: 3, md: 4 }, px: { xs: 2, sm: 3, md: 4 } }}>
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
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            {stats.shift ? (
              <Login sx={{ color: 'success.main' }} />
            ) : (
              <Logout sx={{ color: 'warning.main' }} />
            )}
            <Box>
              <Typography variant="subtitle1" fontWeight={600}>
                {stats.shift ? 'Shift active' : 'No active shift'}
              </Typography>
              {stats.shift && (
                <Typography variant="body2" color="text.secondary">
                  Started {formatDisplayDateTime(stats.shift.startDate)}
                  {shiftDuration && ` \u2022 Duration: ${shiftDuration}`}
                </Typography>
              )}
            </Box>
          </Box>
          {!stats.shift && (
            <Button
              variant="contained"
              onClick={() => void handleStartShift()}
              disabled={startingShift}
            >
              {startingShift ? 'Starting…' : 'Start shift'}
            </Button>
          )}
        </CardContent>
      </Card>

      {stats.shift && (
        <Box sx={{ mb: 4 }}>
          <Typography variant="subtitle2" color="text.secondary" sx={{ mb: 1.5 }}>
            Quick actions
          </Typography>
          <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
            {quickActions.map((action) => {
              const Icon = action.icon;
              return (
                <Button
                  key={action.path}
                  component={RouterLink}
                  to={action.path}
                  variant={action.variant}
                  startIcon={<Icon />}
                >
                  {action.label}
                </Button>
              );
            })}
          </Stack>
        </Box>
      )}

      <Box sx={{ mb: 4 }}>
        <Typography variant="h3" fontWeight={700} gutterBottom>
          Staff dashboard
        </Typography>
        <Typography variant="body1" color="text.secondary">
          {stats.shift
            ? 'Showing collections for your current shift'
            : 'Start a shift to track collections and use counter actions'}
        </Typography>
      </Box>

      <Typography variant="h6" fontWeight={600} sx={{ mb: 2 }}>
        {stats.shift ? 'Shift collection summary' : 'Collections'}
      </Typography>
      {!stats.shift && (
        <Alert severity="info" sx={{ mb: 2 }}>
          Start a shift to track collections for this period.
        </Alert>
      )}
      <Divider sx={{ mb: 3 }} />

      <Grid container spacing={3}>
        {collectionCards.map((card) => {
          const Icon = card.icon;
          return (
            <Grid item xs={12} sm={6} md={4} key={card.title}>
              <Card variant="outlined" sx={{ height: '100%' }}>
                <CardContent>
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
                    <Icon sx={{ color: card.color }} />
                    <Typography variant="subtitle2" color="text.secondary">
                      {card.title}
                    </Typography>
                  </Box>
                  <Typography variant="h5" fontWeight={700} gutterBottom>
                    {card.value}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    {card.subtitle}
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
          );
        })}
      </Grid>
    </Box>
  );
}
