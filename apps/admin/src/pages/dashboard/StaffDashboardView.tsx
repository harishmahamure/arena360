import {
  AccessTime,
  AttachMoney,
  Devices,
  MonetizationOn,
  People,
  ShoppingCart,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  GridLegacy as Grid,
  Typography,
} from '@mui/material';
import { startOfMonth, subDays } from 'date-fns';
import { useMemo, useState } from 'react';
import { useStaffDashboardStats } from '../../hooks/useStaffDashboardStats';
import { formatStatsDate } from '../../services/stats/formatStatsDate';

export default function StaffDashboardView() {
  const [dateRange, setDateRange] = useState<'today' | 'last 7 days' | 'month'>('today');

  const dateFilters = useMemo(() => {
    const now = new Date();

    switch (dateRange) {
      case 'today': {
        const startOfToday = new Date(now);
        startOfToday.setHours(0, 0, 0, 0);
        return {
          startDate: formatStatsDate(startOfToday),
          endDate: formatStatsDate(now),
        };
      }
      case 'last 7 days':
        return {
          startDate: formatStatsDate(subDays(now, 7)),
          endDate: formatStatsDate(now),
        };
      case 'month': {
        const monthStart = startOfMonth(now);
        monthStart.setHours(0, 0, 0, 0);
        return {
          startDate: formatStatsDate(monthStart),
          endDate: formatStatsDate(now),
        };
      }
      default:
        return undefined;
    }
  }, [dateRange]);

  const { data: stats, isLoading, error } = useStaffDashboardStats(dateFilters);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

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

  const cards = [
    {
      title: 'Revenue This Shift',
      value: formatCurrency(stats.shiftRevenue?.total ?? 0),
      subtitle: `Cash: ${formatCurrency(stats.shiftRevenue?.cashRevenue ?? 0)} | Online: ${formatCurrency(stats.shiftRevenue?.onlineRevenue ?? 0)}`,
      icon: MonetizationOn,
      color: '#059669',
    },
    {
      title: 'Revenue Today',
      value: formatCurrency(stats.revenue.total),
      subtitle: `Cash: ${formatCurrency(stats.revenue.cashRevenue)} | Online: ${formatCurrency(stats.revenue.onlineRevenue)}`,
      icon: AttachMoney,
      color: '#10B981',
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
      subtitle: `New this period: ${stats.players.newPlayersInPeriod}`,
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
            {stats.period.label}
          </Typography>
          {stats.shift && (
            <Typography variant="body2" color="text.secondary">
              Shift started {new Date(stats.shift.startDate).toLocaleString()}
            </Typography>
          )}
        </Box>
        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
          {(['today', 'last 7 days', 'month'] as const).map((range) => (
            <Button
              key={range}
              size="small"
              variant={dateRange === range ? 'contained' : 'outlined'}
              onClick={() => setDateRange(range)}
            >
              {range === 'last 7 days'
                ? 'Last 7 Days'
                : range.charAt(0).toUpperCase() + range.slice(1)}
            </Button>
          ))}
        </Box>
      </Box>

      <Grid container spacing={3}>
        {cards.map((card) => {
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
