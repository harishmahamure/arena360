import { round } from '@gaming-cafe/utils';
import {
  AccessTime,
  AttachMoney,
  Devices,
  MonetizationOn,
  People,
  ShoppingCart,
  SportsEsports,
  TrendingDown,
  TrendingUp,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CardHeader,
  Chip,
  CircularProgress,
  Divider,
  GridLegacy as Grid,
  LinearProgress,
  Typography,
} from '@mui/material';
import { subDays } from 'date-fns';
import { useMemo, useState } from 'react';
import { useDashboardStats } from '../../hooks/useDashboardStats';
import { formatStatsDate } from '../../services/stats/formatStatsDate';
import { endOfTodayIST, now, startOfTodayIST, toISTString } from '../../utils/date';

export default function AdminDashboardView() {
  const [dateRange, setDateRange] = useState<'today' | 'last 7 days' | 'month' | 'all'>('today');

  const dateFilters = useMemo(() => {
    const current = now();

    switch (dateRange) {
      case 'today':
        return {
          startDate: toISTString(startOfTodayIST()),
          endDate: toISTString(endOfTodayIST()),
        };
      case 'last 7 days':
        return {
          startDate: formatStatsDate(subDays(current, 7)),
          endDate: toISTString(endOfTodayIST()),
        };
      case 'month': {
        const d = new Date(current);
        d.setDate(1);
        d.setHours(0, 0, 0, 0);
        return {
          startDate: formatStatsDate(d),
          endDate: toISTString(endOfTodayIST()),
        };
      }
      default:
        return undefined;
    }
  }, [dateRange]);

  const { data: dashboardStats, isLoading, error } = useDashboardStats(dateFilters);

  const formatCurrency = (amount: number) => {
    return new Intl.NumberFormat('en-IN', {
      style: 'currency',
      currency: 'INR',
    }).format(amount);
  };

  const calculateChange = (current: number, previous?: number) => {
    const change = current - (previous ?? 0);
    return {
      value: round((change / (previous ?? 1)) * 100, 1),
      positive: change >= 0,
    };
  };

  if (isLoading) {
    return (
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          minHeight: '80vh',
        }}
      >
        <CircularProgress />
      </Box>
    );
  }

  if (error) {
    return (
      <Box sx={{ py: 4, px: 3 }}>
        <Alert severity="error">Failed to load dashboard statistics. Please try again later.</Alert>
      </Box>
    );
  }

  const stats = dashboardStats
    ? [
        {
          title: 'Total Revenue',
          value: formatCurrency(dashboardStats?.revenue?.current?.total),
          change: calculateChange(
            dashboardStats?.revenue?.current?.total,
            dashboardStats?.revenue?.previous?.total,
          ),
          icon: AttachMoney,
          color: '#10B981',
          subtitle: `Merchandise: ${formatCurrency(
            dashboardStats?.revenue?.current?.merchandise,
          )} | Gaming: ${formatCurrency(dashboardStats?.revenue?.current?.plan)}`,
        },
        {
          title: 'Cash Revenue',
          value: `${formatCurrency(dashboardStats?.revenue?.current?.cashRevenue)}`,
          change: calculateChange(
            dashboardStats?.revenue?.current?.cashRevenue,
            dashboardStats?.revenue?.previous?.cashRevenue,
          ),
          subtitle: `Previous Period: ${formatCurrency(
            dashboardStats?.revenue?.previous?.cashRevenue,
          )}`,
          icon: MonetizationOn,
          color: '#10B981',
        },
        {
          title: 'Online Revenue',
          value: `${formatCurrency(dashboardStats?.revenue?.current?.onlineRevenue)}`,
          change: calculateChange(
            dashboardStats?.revenue?.current?.onlineRevenue,
            dashboardStats?.revenue?.previous?.onlineRevenue,
          ),
          subtitle: `Previous Period: ${formatCurrency(
            dashboardStats?.revenue?.previous?.onlineRevenue,
          )}`,
          icon: MonetizationOn,
          color: '#10B981',
        },
        {
          title: 'Total Transactions',
          value: dashboardStats.transactions?.current?.completedTransactions?.toLocaleString(),
          change: calculateChange(
            dashboardStats.transactions?.current?.completedTransactions,
            dashboardStats.transactions?.previous?.completedTransactions,
          ),
          icon: ShoppingCart,
          color: '#3B82F6',
          subtitle: `Avg: ${formatCurrency(
            dashboardStats.transactions?.current?.averageTransactionAmount,
          )}`,
        },
        {
          title: 'Active Players',
          value: dashboardStats.users.activePlayers.toLocaleString(),
          change: calculateChange(dashboardStats.users.activePlayers),
          icon: People,
          color: '#8B5CF6',
          subtitle: `Total: ${dashboardStats.users.totalPlayers.toLocaleString()}`,
        },
        {
          title: 'Active Sessions',
          value: dashboardStats.usage?.current?.activeSessions?.toLocaleString(),
          change: calculateChange(
            dashboardStats.usage?.current?.activeSessions,
            dashboardStats.usage?.previous?.activeSessions,
          ),
          icon: AccessTime,
          color: '#F59E0B',
          subtitle: `Total: ${dashboardStats.usage?.current?.totalSessions?.toLocaleString()}`,
        },
        {
          title: 'Active Devices',
          value: dashboardStats.devices.activeDevices.toLocaleString(),
          change: calculateChange(dashboardStats.devices.activeDevices),
          icon: Devices,
          color: '#EF4444',
          subtitle: `Total: ${dashboardStats.devices.totalDevices.toLocaleString()}`,
        },
        {
          title: 'Total Usage Hours',
          value: dashboardStats.usage?.current?.totalHours?.toLocaleString(),
          change: calculateChange(
            dashboardStats.usage?.current?.totalHours,
            dashboardStats.usage?.previous?.totalHours,
          ),
          icon: SportsEsports,
          color: '#06B6D4',
          subtitle: `Avg: ${dashboardStats.usage?.current?.averageSessionDuration?.toFixed(1)} min`,
        },
      ]
    : [];

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
            Dashboard
          </Typography>

          <Typography variant="body1" color="text.secondary">
            {dashboardStats?.period?.label}
          </Typography>
          <Typography variant="body1" color="text.secondary">
            compared to {dashboardStats?.period?.previousLabel}
          </Typography>
        </Box>
        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
          <Button
            size="small"
            variant={dateRange === 'today' ? 'contained' : 'outlined'}
            onClick={() => setDateRange('today')}
          >
            Today
          </Button>
          <Button
            size="small"
            variant={dateRange === 'last 7 days' ? 'contained' : 'outlined'}
            onClick={() => setDateRange('last 7 days')}
          >
            Last 7 Days
          </Button>
          <Button
            size="small"
            variant={dateRange === 'month' ? 'contained' : 'outlined'}
            onClick={() => setDateRange('month')}
          >
            MTD
          </Button>
        </Box>
      </Box>

      {/* Stats Cards */}
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {stats.map((stat) => (
          <Grid item xs={12} sm={6} md={4} key={stat.title}>
            <Card className="hover-lift">
              <CardContent>
                <Box
                  sx={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'flex-start',
                    mb: 2,
                  }}
                >
                  <Box
                    sx={{
                      width: 48,
                      height: 48,
                      borderRadius: 2,
                      bgcolor: `${stat.color}15`,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                    }}
                  >
                    <stat.icon sx={{ color: stat.color, fontSize: 24 }} />
                  </Box>
                  <Chip
                    size="small"
                    icon={stat.change.positive ? <TrendingUp /> : <TrendingDown />}
                    label={`${stat.change.positive ? '+' : ''}${stat.change.value}%`}
                    color={stat.change.positive ? 'success' : 'error'}
                  />
                </Box>
                <Typography variant="h4" fontWeight={700} sx={{ mb: 0.5 }}>
                  {stat.value}
                </Typography>
                <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
                  {stat.title}
                </Typography>
                {stat.subtitle && (
                  <Typography variant="caption" color="text.secondary">
                    {stat.subtitle}
                  </Typography>
                )}
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>

      {/* Top Performers Section */}
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {/* Top Plans */}
        <Grid item xs={12} md={4}>
          <Card>
            <CardHeader title="Top Plans" />
            <Divider />
            <CardContent>
              {dashboardStats?.topPerformers.topPlans.slice(0, 5).map((plan, index) => (
                <Box
                  key={plan.planId}
                  sx={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    mb: 2,
                  }}
                >
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                    <Chip
                      label={index + 1}
                      size="small"
                      color={index < 3 ? 'primary' : 'default'}
                    />
                    <Box>
                      <Typography variant="body2" fontWeight={500}>
                        {plan.planName}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {plan.purchaseCount} purchases
                      </Typography>
                    </Box>
                  </Box>
                  <Typography variant="body2" fontWeight={600}>
                    {formatCurrency(plan.revenue)}
                  </Typography>
                </Box>
              ))}
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={4}>
          <Card>
            <CardHeader title="Top Players" />
            <Divider />
            <CardContent>
              {dashboardStats?.topPerformers.topPlayers.slice(0, 5).map((player, index) => (
                <Box
                  key={player.playerId}
                  sx={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    mb: 2,
                  }}
                >
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                    <Chip
                      label={index + 1}
                      size="small"
                      color={index < 3 ? 'primary' : 'default'}
                    />
                    <Box>
                      <Typography variant="body2" fontWeight={500}>
                        {player.playerName}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {player.totalSessions} sessions
                      </Typography>
                    </Box>
                  </Box>
                  <Typography variant="body2" fontWeight={600}>
                    {formatCurrency(player.totalSpent)}
                  </Typography>
                </Box>
              ))}
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Device Utilization */}
      <Card>
        <CardHeader title="Device Utilization" />
        <Divider />
        <CardContent>
          {dashboardStats?.devices.deviceUtilization.slice(0, 10).map((device) => (
            <Box key={device.deviceId} sx={{ mb: 3 }}>
              <Box
                sx={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  mb: 1,
                }}
              >
                <Box>
                  <Typography variant="body2" fontWeight={500}>
                    {device.deviceName}
                  </Typography>
                  <Typography variant="caption" color="text.secondary">
                    {device.totalSessions} sessions • {device.totalHours.toFixed(1)} hours
                  </Typography>
                </Box>
                <Typography variant="body2" fontWeight={600}>
                  {device.utilizationPercentage.toFixed(1)}%
                </Typography>
              </Box>
              <LinearProgress
                variant="determinate"
                value={device.utilizationPercentage}
                color={
                  device.utilizationPercentage > 80
                    ? 'success'
                    : device.utilizationPercentage > 50
                      ? 'warning'
                      : 'error'
                }
                sx={{ height: 8, borderRadius: 4 }}
              />
            </Box>
          ))}
        </CardContent>
      </Card>
    </Box>
  );
}
