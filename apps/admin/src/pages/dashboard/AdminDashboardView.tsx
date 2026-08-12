import { ErrorPanel, PageHeader, PageShell } from '@gaming-cafe/ui';
import {
  AccessTime,
  AttachMoney,
  CreditCard,
  Devices,
  MonetizationOn,
  People,
  Receipt,
  ShoppingCart,
  SportsEsports,
} from '@mui/icons-material';
import { Box, Card, CardContent, Grid, LinearProgress, Skeleton, Typography } from '@mui/material';
import { StatCard, type StatTone } from '../../containers/stats/StatCard';
import { StatsDateRangeToolbar } from '../../containers/stats/StatsDateRangeToolbar';
import { TopPerformersList } from '../../containers/stats/TopPerformersList';
import { useDashboardStats } from '../../hooks/useDashboardStats';
import { useStatsDateRange } from '../../hooks/useStatsDateRange';
import {
  calculatePeriodChange,
  formatPaymentCountBreakdown,
  formatTypePaymentSubtitle,
  normalizeRevenue,
} from '../../services/stats/statsHelpers';
import type { RevenueByPaymentMethodDto } from '../../services/stats/types';

function AdminDashboardSkeleton() {
  return (
    <PageShell>
      <Skeleton variant="text" width={180} height={40} sx={{ mb: 1 }} />
      <Skeleton variant="text" width={280} height={24} sx={{ mb: 2 }} />
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mb: 3 }}>
        {['dr-1', 'dr-2', 'dr-3'].map((id) => (
          <Skeleton key={id} variant="rounded" width={72} height={36} />
        ))}
      </Box>
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {['stat-1', 'stat-2', 'stat-3', 'stat-4', 'stat-5', 'stat-6', 'stat-7', 'stat-8'].map(
          (id) => (
            <Grid key={id} size={{ xs: 12, sm: 6, md: 4 }}>
              <Skeleton variant="rounded" height={160} />
            </Grid>
          ),
        )}
      </Grid>
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {['perf-1', 'perf-2'].map((id) => (
          <Grid key={id} size={{ xs: 12, md: 6 }}>
            <Skeleton variant="rounded" height={220} />
          </Grid>
        ))}
      </Grid>
      <Skeleton variant="rounded" height={280} />
    </PageShell>
  );
}

export default function AdminDashboardView() {
  const {
    startDate,
    endDate,
    compare,
    appliedCompare,
    apiFilters,
    isDirty,
    setRange,
    setCompare,
    applyPreset,
    apply,
  } = useStatsDateRange();

  const { data: dashboardStats, isLoading, error, refetch } = useDashboardStats(apiFilters);

  const formatCurrency = (amount: number) => {
    return new Intl.NumberFormat('en-IN', {
      style: 'currency',
      currency: 'INR',
    }).format(amount);
  };

  const formatPaymentBreakdown = (revenue: RevenueByPaymentMethodDto) =>
    `Cash: ${formatCurrency(revenue.cashRevenue)} | Online: ${formatCurrency(revenue.onlineRevenue)} | Credit: ${formatCurrency(revenue.creditRevenue)}`;

  const change = (current: number, previous?: number | null) =>
    appliedCompare ? calculatePeriodChange(current, previous ?? undefined) : undefined;

  if (isLoading) {
    return <AdminDashboardSkeleton />;
  }

  if (error || !dashboardStats) {
    return (
      <PageShell>
        <ErrorPanel
          message="Failed to load dashboard statistics. Please try again later."
          onRetry={() => void refetch()}
        />
      </PageShell>
    );
  }

  const currentRevenue = normalizeRevenue(dashboardStats.revenue.current);
  const previousRevenue = dashboardStats.revenue.previous
    ? normalizeRevenue(dashboardStats.revenue.previous)
    : null;

  const stats = [
    {
      title: 'Total Revenue',
      value: formatCurrency(currentRevenue.total),
      change: change(currentRevenue.total, previousRevenue?.total),
      icon: AttachMoney,
      tone: 'success' as StatTone,
      subtitle: formatPaymentBreakdown(currentRevenue),
    },
    {
      title: 'Plan Transactions',
      value: formatCurrency(currentRevenue.plan),
      change: change(currentRevenue.plan, previousRevenue?.plan),
      icon: Receipt,
      tone: 'primary' as StatTone,
      subtitle: formatTypePaymentSubtitle(currentRevenue, 'plan', formatCurrency),
    },
    {
      title: 'Product Transactions',
      value: formatCurrency(currentRevenue.merchandise),
      change: change(currentRevenue.merchandise, previousRevenue?.merchandise),
      icon: ShoppingCart,
      tone: 'info' as StatTone,
      subtitle: formatTypePaymentSubtitle(currentRevenue, 'product', formatCurrency),
    },
    {
      title: 'Cash Revenue',
      value: formatCurrency(currentRevenue.cashRevenue),
      change: change(currentRevenue.cashRevenue, previousRevenue?.cashRevenue),
      subtitle: previousRevenue
        ? `Previous Period: ${formatCurrency(previousRevenue.cashRevenue)}`
        : undefined,
      icon: MonetizationOn,
      tone: 'success' as StatTone,
    },
    {
      title: 'Online Revenue',
      value: formatCurrency(currentRevenue.onlineRevenue),
      change: change(currentRevenue.onlineRevenue, previousRevenue?.onlineRevenue),
      subtitle: previousRevenue
        ? `Previous Period: ${formatCurrency(previousRevenue.onlineRevenue)}`
        : undefined,
      icon: MonetizationOn,
      tone: 'success' as StatTone,
    },
    {
      title: 'Credit Revenue',
      value: formatCurrency(currentRevenue.creditRevenue),
      change: change(currentRevenue.creditRevenue, previousRevenue?.creditRevenue),
      subtitle: previousRevenue
        ? `Previous Period: ${formatCurrency(previousRevenue.creditRevenue)}`
        : undefined,
      icon: CreditCard,
      tone: 'warning' as StatTone,
    },
    {
      title: 'Total Transactions',
      value: dashboardStats.transactions.current.completedTransactions.toLocaleString(),
      change: change(
        dashboardStats.transactions.current.completedTransactions,
        dashboardStats.transactions.previous?.completedTransactions,
      ),
      icon: ShoppingCart,
      tone: 'info' as StatTone,
      subtitle: `Avg: ${formatCurrency(dashboardStats.transactions.current.averageTransactionAmount)}\n${formatPaymentCountBreakdown(currentRevenue)}`,
    },
    {
      title: 'Active Players',
      value: dashboardStats.users.activePlayers.toLocaleString(),
      change: change(dashboardStats.users.activePlayers),
      icon: People,
      tone: 'primary' as StatTone,
      subtitle: `Total: ${dashboardStats.users.totalPlayers.toLocaleString()}`,
    },
    {
      title: 'Active Sessions',
      value: dashboardStats.usage.current.activeSessions.toLocaleString(),
      change: change(
        dashboardStats.usage.current.activeSessions,
        dashboardStats.usage.previous?.activeSessions,
      ),
      icon: AccessTime,
      tone: 'warning' as StatTone,
      subtitle: `Total: ${dashboardStats.usage.current.totalSessions.toLocaleString()}`,
    },
    {
      title: 'Active Devices',
      value: dashboardStats.devices.activeDevices.toLocaleString(),
      change: change(dashboardStats.devices.activeDevices),
      icon: Devices,
      tone: 'error' as StatTone,
      subtitle: `Total: ${dashboardStats.devices.totalDevices.toLocaleString()}`,
    },
    {
      title: 'Total Usage Hours',
      value: dashboardStats.usage.current.totalHours.toLocaleString(),
      change: change(
        dashboardStats.usage.current.totalHours,
        dashboardStats.usage.previous?.totalHours,
      ),
      icon: SportsEsports,
      tone: 'info' as StatTone,
      shade: 'dark' as const,
      subtitle: `Avg: ${dashboardStats.usage.current.averageSessionDuration.toFixed(1)} min`,
    },
  ];

  const topPlans = dashboardStats.topPerformers.topPlans.slice(0, 5).map((plan) => ({
    id: plan.planId,
    name: plan.planName,
    primaryMetric: formatCurrency(plan.revenue),
    secondaryMetric: plan.purchaseCount,
  }));

  const topPlayers = dashboardStats.topPerformers.topPlayers.slice(0, 5).map((player) => ({
    id: player.playerId,
    name: player.playerName,
    primaryMetric: formatCurrency(player.totalSpent),
    secondaryMetric: player.totalSessions,
  }));

  const deviceUtilization = dashboardStats.devices.deviceUtilization.slice(0, 10);

  const description =
    appliedCompare && dashboardStats.period.previousLabel
      ? `${dashboardStats.period.label} · compared to ${dashboardStats.period.previousLabel}`
      : dashboardStats.period.label;

  return (
    <PageShell
      header={<PageHeader title="Dashboard" description={description} />}
      toolbar={
        <StatsDateRangeToolbar
          startDate={startDate}
          endDate={endDate}
          compare={compare}
          onRangeChange={setRange}
          onCompareChange={setCompare}
          onPreset={applyPreset}
          onApply={apply}
          isDirty={isDirty}
        />
      }
    >
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {stats.map((stat) => (
          <Grid key={stat.title} size={{ xs: 12, sm: 6, md: 4 }}>
            <StatCard
              title={stat.title}
              value={stat.value}
              subtitle={stat.subtitle}
              change={stat.change}
              tone={stat.tone}
              shade={stat.shade}
              icon={<stat.icon sx={{ fontSize: 24 }} />}
            />
          </Grid>
        ))}
      </Grid>

      <Grid container spacing={3} sx={{ mb: 4 }}>
        <Grid size={{ xs: 12, md: 6 }}>
          <TopPerformersList title="Top Plans" items={topPlans} secondaryLabel="Purchases" />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <TopPerformersList title="Top Players" items={topPlayers} secondaryLabel="Sessions" />
        </Grid>
      </Grid>

      <Card variant="outlined">
        <Typography variant="subtitle1" fontWeight={600} sx={{ px: 2, pt: 2 }}>
          Device utilization
        </Typography>
        <CardContent>
          {deviceUtilization.length === 0 ? (
            <Typography variant="body2" color="text.secondary" textAlign="center">
              No utilization data for this period
            </Typography>
          ) : (
            deviceUtilization.map((device) => (
              <Box key={device.deviceId} sx={{ mb: 3, '&:last-child': { mb: 0 } }}>
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
                      {device.totalSessions} sessions · {device.totalHours.toFixed(1)} hours
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
            ))
          )}
        </CardContent>
      </Card>
    </PageShell>
  );
}
