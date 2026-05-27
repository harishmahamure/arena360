import {
  AccessTime,
  Devices,
  Login,
  Logout,
  People,
  PointOfSale,
  ShoppingCart,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Card,
  CardContent,
  CircularProgress,
  Divider,
  GridLegacy as Grid,
  Typography,
} from '@mui/material';
import { useMemo } from 'react';
import { useStaffDashboardStats } from '../../hooks/useStaffDashboardStats';
import { formatDisplayDateTime, formatDuration, now as nowDate } from '../../utils/date';

export default function StaffDashboardView() {
  const { data: stats, isLoading, error } = useStaffDashboardStats();

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
      {/* Shift Header Bar */}
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
                {stats.shift ? 'Shift Active' : 'No Active Shift'}
              </Typography>
              {stats.shift && (
                <Typography variant="body2" color="text.secondary">
                  Started {formatDisplayDateTime(stats.shift.startDate)}
                  {shiftDuration && ` \u2022 Duration: ${shiftDuration}`}
                </Typography>
              )}
            </Box>
          </Box>
        </CardContent>
      </Card>

      <Box
        sx={{
          display: 'flex',
          flexDirection: { xs: 'column', sm: 'row' },
          justifyContent: 'space-between',
          alignItems: { xs: 'flex-start', sm: 'center' },
          gap: 2,
          mb: 4,
        }}
      >
        <Box>
          <Typography variant="h3" fontWeight={700} gutterBottom>
            Staff Dashboard
          </Typography>
          <Typography variant="body1" color="text.secondary">
            {stats.shift ? 'Showing collections for your current shift' : stats.period.label}
          </Typography>
        </Box>
      </Box>

      {/* Collection Summary */}
      <Typography variant="h6" fontWeight={600} sx={{ mb: 2 }}>
        Shift Collection Summary
      </Typography>
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
