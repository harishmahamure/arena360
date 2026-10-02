import { ErrorPanel, PageHeader, PageShell } from '@gaming-cafe/ui';
import {
  ArrowForward,
  ArrowOutward,
  CreditCardOutlined,
  GridViewOutlined,
  PaymentsOutlined,
  PeopleOutline,
  PlayCircleOutline,
  ReceiptLongOutlined,
  Refresh,
  SettingsOutlined,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  Chip,
  Divider,
  Grid,
  IconButton,
  LinearProgress,
  Skeleton,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link as RouterLink } from 'react-router-dom';
import { StatCard } from '../../containers/stats/StatCard';
import { StatsDateRangeToolbar } from '../../containers/stats/StatsDateRangeToolbar';
import { TopPerformersList } from '../../containers/stats/TopPerformersList';
import { useDashboardStats } from '../../hooks/useDashboardStats';
import { useStatsDateRange } from '../../hooks/useStatsDateRange';
import { getInventoryOverview, getPurchaseOrders } from '../../services/inventory';
import { calculatePeriodChange, normalizeRevenue } from '../../services/stats/statsHelpers';
import type { RevenueTrendDto } from '../../services/stats/types';

const money = (amount: number) =>
  new Intl.NumberFormat('en-IN', {
    style: 'currency',
    currency: 'INR',
    maximumFractionDigits: 0,
  }).format(amount);

function RevenueChart({ rows }: { rows: RevenueTrendDto[] }) {
  const max = Math.max(...rows.map((row) => row.totalRevenue), 1);
  if (!rows.length)
    return (
      <Box sx={{ minHeight: 245, display: 'grid', placeItems: 'center' }}>
        <Typography color="text.secondary">
          Revenue will appear here as sales are completed.
        </Typography>
      </Box>
    );
  // Plot each observed period without inventing intermediate revenue values.
  const points = rows.map((row, i) => ({
    x: rows.length === 1 ? 345 : 55 + (i / (rows.length - 1)) * 585,
    y: 200 - (row.totalRevenue / max) * 165,
  }));
  const path = points.map((point, i) => `${i === 0 ? 'M' : 'L'} ${point.x} ${point.y}`).join(' ');
  return (
    <Box sx={{ mt: 2, width: '100%', overflow: 'hidden' }}>
      <svg
        viewBox="0 0 680 240"
        role="img"
        aria-label={`Revenue across ${rows.length} periods. Total ${money(rows.reduce((sum, row) => sum + row.totalRevenue, 0))}.`}
        style={{ width: '100%', display: 'block' }}
      >
        {[0, 1, 2, 3].map((i) => (
          <g key={i}>
            <line
              x1="55"
              y1={35 + i * 55}
              x2="645"
              y2={35 + i * 55}
              stroke="#e9eeec"
              strokeDasharray="4 4"
            />
            <text x="44" y={39 + i * 55} textAnchor="end" fontSize="10" fill="#809189">
              {new Intl.NumberFormat('en', {
                notation: 'compact',
                maximumFractionDigits: 1,
              }).format(max * (1 - i / 3))}
            </text>
          </g>
        ))}
        {points.length > 1 && (
          <path d={`${path} L ${points.at(-1)?.x} 200 L ${points[0]?.x} 200 Z`} fill="#176b5110" />
        )}
        <path d={path} fill="none" stroke="#268268" strokeWidth="2.5" strokeLinejoin="round" />
        {rows.map((row, i) => (
          <g key={row.date}>
            <circle
              cx={points[i]?.x}
              cy={points[i]?.y}
              r="4"
              fill="#fff"
              stroke="#268268"
              strokeWidth="2"
            >
              <title>
                {row.date}: {money(row.totalRevenue)} · {row.transactionCount} transactions
              </title>
            </circle>
            {(i === 0 || i === rows.length - 1 || i === Math.floor(rows.length / 2)) && (
              <text x={points[i]?.x} y="226" textAnchor="middle" fontSize="10" fill="#809189">
                {new Date(row.date).toLocaleDateString('en-IN', { month: 'short', day: 'numeric' })}
              </text>
            )}
          </g>
        ))}
      </svg>
    </Box>
  );
}

export default function AdminDashboardView() {
  const inventory = useQuery({ queryKey: ['inventory-overview'], queryFn: getInventoryOverview });
  const orders = useQuery({
    queryKey: ['purchase-orders', 'submitted', 1],
    queryFn: () => getPurchaseOrders({ status: 'submitted', page: 1, limit: 1 }),
  });
  const range = useStatsDateRange();
  const { data, isLoading, error, refetch, isFetching } = useDashboardStats(range.apiFilters);
  const header = (
    <Stack direction="row" justifyContent="space-between" alignItems="flex-start" gap={2}>
      <PageHeader
        title="Overview"
        description="A clear view of your business. A better start to your day."
      />
      <Stack direction="row" spacing={1}>
        <Tooltip title="Refresh dashboard">
          <IconButton
            aria-label="Refresh dashboard"
            disabled={isFetching}
            onClick={() => {
              void refetch();
              void inventory.refetch();
              void orders.refetch();
            }}
          >
            <Refresh fontSize="small" />
          </IconButton>
        </Tooltip>
        <Button
          component={RouterLink}
          to="/stations"
          variant="outlined"
          startIcon={<GridViewOutlined />}
          sx={{ whiteSpace: 'nowrap', display: { xs: 'none', sm: 'flex' } }}
        >
          View floor
        </Button>
      </Stack>
    </Stack>
  );
  if (isLoading)
    return (
      <PageShell header={header}>
        <Grid container spacing={2}>
          {[1, 2, 3, 4].map((n) => (
            <Grid key={n} size={{ xs: 12, sm: 6, lg: 3 }}>
              <Skeleton variant="rounded" height={156} />
            </Grid>
          ))}
          <Grid size={12}>
            <Skeleton variant="rounded" height={350} />
          </Grid>
        </Grid>
      </PageShell>
    );
  if (error || !data)
    return (
      <PageShell header={header}>
        <ErrorPanel
          message="We couldn’t load your overview. Check your connection and try again."
          onRetry={() => void refetch()}
        />
      </PageShell>
    );
  const revenue = normalizeRevenue(data.revenue.current);
  const previous = data.revenue.previous ? normalizeRevenue(data.revenue.previous) : undefined;
  const change = (current: number, prior?: number) =>
    range.appliedCompare && prior !== undefined && prior !== 0
      ? calculatePeriodChange(current, prior)
      : undefined;
  const alerts = [
    {
      label: 'Purchase orders to approve',
      value: orders.data?.total,
      path: '/inventory/purchase-orders?status=submitted',
      hint: 'Review supplier orders',
      failed: orders.isError,
    },
    {
      label: 'Products running low',
      value: inventory.data?.lowStockProducts,
      path: '/inventory/reorder',
      hint: 'Plan your next restock',
      failed: inventory.isError,
    },
    {
      label: 'Products out of stock',
      value: inventory.data?.outOfStockProducts,
      path: '/inventory/stock',
      hint: 'Check stock availability',
      failed: inventory.isError,
    },
    {
      label: 'Transfers & waste to review',
      value: inventory.data
        ? inventory.data.pendingTransfers + inventory.data.pendingWasteEvents
        : undefined,
      path: '/inventory',
      hint: 'Keep inventory up to date',
      failed: inventory.isError,
    },
  ];
  return (
    <PageShell header={header}>
      <Box
        sx={{
          bgcolor: 'background.paper',
          p: 2,
          border: 1,
          borderColor: 'divider',
          borderRadius: 2,
          mb: 3,
        }}
      >
        <StatsDateRangeToolbar
          startDate={range.startDate}
          endDate={range.endDate}
          compare={range.compare}
          onRangeChange={range.setRange}
          onCompareChange={range.setCompare}
          onPreset={range.applyPreset}
          onApply={range.apply}
          isDirty={range.isDirty}
        />
      </Box>
      <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mb: 2 }}>
        <Typography variant="overline" color="text.secondary">
          PERFORMANCE SNAPSHOT
        </Typography>
        <Typography variant="caption" color="text.secondary">
          · {data.period.label}
          {range.appliedCompare && data.period.previousLabel
            ? ` vs ${data.period.previousLabel}`
            : ''}
        </Typography>
      </Box>
      <Grid container spacing={2} sx={{ mb: 3 }}>
        <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
          <StatCard
            title="Total revenue"
            value={money(revenue.total)}
            subtitle={`${data.transactions.current.completedTransactions} completed transactions`}
            change={change(revenue.total, previous?.total)}
            icon={<PaymentsOutlined />}
            tone="primary"
          />
        </Grid>
        <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
          <StatCard
            title="Active sessions"
            value={data.usage.current.activeSessions}
            subtitle={`${data.usage.current.totalSessions} sessions in this period`}
            change={change(data.usage.current.activeSessions, data.usage.previous?.activeSessions)}
            icon={<PlayCircleOutline />}
            tone="info"
          />
        </Grid>
        <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
          <StatCard
            title="Active players"
            value={data.users.activePlayers}
            subtitle={`${data.users.totalPlayers} players in your community`}
            icon={<PeopleOutline />}
            tone="primary"
          />
        </Grid>
        <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
          <StatCard
            title="Average transaction"
            value={money(data.transactions.current.averageTransactionAmount)}
            subtitle={`${data.usage.current.totalHours.toFixed(1)} total hours played`}
            change={change(
              data.transactions.current.averageTransactionAmount,
              data.transactions.previous?.averageTransactionAmount,
            )}
            icon={<ReceiptLongOutlined />}
            tone="warning"
          />
        </Grid>
      </Grid>
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid size={{ xs: 12, lg: 8 }}>
          <Card sx={{ p: 3, height: '100%' }}>
            <Stack direction="row" justifyContent="space-between" alignItems="flex-start">
              <Box>
                <Typography variant="h6">Revenue performance</Typography>
                <Typography variant="caption" color="text.secondary">
                  Completed sales across the selected period · INR
                </Typography>
              </Box>
              <Chip label={data.period.label} variant="outlined" />
            </Stack>
            <Typography sx={{ mt: 2, fontSize: 30, letterSpacing: '-.04em', fontWeight: 650 }}>
              {money(revenue.total)}
            </Typography>
            <RevenueChart rows={data.revenueTrend ?? []} />
            <Divider sx={{ my: 2 }} />
            <Stack direction="row" spacing={4} useFlexGap flexWrap="wrap">
              <Box>
                <Typography variant="caption" color="text.secondary">
                  Gaming plans
                </Typography>
                <Typography fontWeight={650}>{money(revenue.plan)}</Typography>
              </Box>
              <Box>
                <Typography variant="caption" color="text.secondary">
                  Products & merchandise
                </Typography>
                <Typography fontWeight={650}>{money(revenue.merchandise)}</Typography>
              </Box>
              <Box sx={{ ml: 'auto' }}>
                <Button
                  component={RouterLink}
                  to="/finance/reconciliation"
                  endIcon={<ArrowForward fontSize="small" />}
                >
                  View finance
                </Button>
              </Box>
            </Stack>
          </Card>
        </Grid>
        <Grid size={{ xs: 12, lg: 4 }}>
          <Card sx={{ p: 3, height: '100%' }}>
            <Stack
              direction="row"
              justifyContent="space-between"
              alignItems="center"
              sx={{ mb: 1 }}
            >
              <Typography variant="h6">Needs attention</Typography>
              <Chip label="Operations" variant="outlined" />
            </Stack>
            <Typography variant="caption" color="text.secondary">
              Your next steps, in one place.
            </Typography>
            {alerts.map((item) => (
              <Box
                key={item.path}
                component={RouterLink}
                to={item.path}
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 1.5,
                  py: 2.5,
                  textDecoration: 'none',
                  color: 'inherit',
                  borderBottom: 1,
                  borderColor: 'divider',
                  '&:last-child': { borderBottom: 0 },
                  '&:hover': { color: 'primary.main' },
                }}
              >
                <Box
                  sx={{
                    width: 38,
                    height: 38,
                    borderRadius: 2,
                    bgcolor: item.value ? '#fff5e4' : '#f1f5f3',
                    display: 'grid',
                    placeItems: 'center',
                    color: item.value ? '#9a6c1e' : '#5d7869',
                    fontWeight: 700,
                  }}
                >
                  {item.failed ? '!' : (item.value ?? '—')}
                </Box>
                <Box sx={{ flex: 1 }}>
                  <Typography fontWeight={600} fontSize={12}>
                    {item.label}
                  </Typography>
                  <Typography variant="caption" color="text.secondary">
                    {item.failed ? 'Could not load · open to retry' : item.hint}
                  </Typography>
                </Box>
                <ArrowOutward sx={{ fontSize: 17, color: 'text.secondary' }} />
              </Box>
            ))}
          </Card>
        </Grid>
      </Grid>
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid size={{ xs: 12, lg: 8 }}>
          <Card sx={{ p: 3 }}>
            <Stack
              direction="row"
              justifyContent="space-between"
              alignItems="center"
              sx={{ mb: 3 }}
            >
              <Box>
                <Typography variant="h6">Your gaming floor</Typography>
                <Typography variant="caption" color="text.secondary">
                  {data.devices.activeDevices} active devices of {data.devices.totalDevices} ·{' '}
                  {data.usage.current.averageSessionDuration.toFixed(0)} min average session
                </Typography>
              </Box>
              <Button component={RouterLink} to="/devices" size="small" endIcon={<ArrowForward />}>
                Devices
              </Button>
            </Stack>
            {data.devices.deviceUtilization.length ? (
              data.devices.deviceUtilization.slice(0, 6).map((device) => (
                <Box key={device.deviceId} sx={{ mb: 2, '&:last-child': { mb: 0 } }}>
                  <Stack direction="row" justifyContent="space-between" sx={{ mb: 1 }}>
                    <Typography fontSize={12} fontWeight={550}>
                      {device.deviceName}
                      <Typography
                        component="span"
                        variant="caption"
                        color="text.secondary"
                        sx={{ ml: 1 }}
                      >
                        {device.totalSessions} sessions
                      </Typography>
                    </Typography>
                    <Typography fontSize={12} fontWeight={600}>
                      {device.utilizationPercentage.toFixed(0)}%
                    </Typography>
                  </Stack>
                  <LinearProgress
                    variant="determinate"
                    value={Math.min(100, Math.max(0, device.utilizationPercentage))}
                  />
                </Box>
              ))
            ) : (
              <Typography color="text.secondary" sx={{ py: 4 }}>
                No device activity in this period.
              </Typography>
            )}
          </Card>
        </Grid>
        <Grid size={{ xs: 12, lg: 4 }}>
          <Card sx={{ p: 3, height: '100%' }}>
            <Typography variant="h6" sx={{ mb: 3 }}>
              Payment breakdown
            </Typography>
            {[
              { label: 'Cash', amount: revenue.cashRevenue, color: '#237a5c' },
              { label: 'Online', amount: revenue.onlineRevenue, color: '#598ab1' },
              { label: 'Running tab', amount: revenue.creditRevenue, color: '#bd944d' },
            ].map((item) => (
              <Stack
                key={item.label}
                direction="row"
                justifyContent="space-between"
                alignItems="center"
                sx={{ py: 1.5 }}
              >
                <Stack direction="row" spacing={1} alignItems="center">
                  <Box sx={{ width: 7, height: 7, borderRadius: '50%', bgcolor: item.color }} />
                  <Typography variant="body2">{item.label}</Typography>
                </Stack>
                <Typography fontWeight={600} fontSize={13}>
                  {money(item.amount)}
                </Typography>
              </Stack>
            ))}
            <Divider sx={{ my: 2 }} />
            <Button
              component={RouterLink}
              to="/credit"
              startIcon={<CreditCardOutlined />}
              size="small"
            >
              Manage running tabs
            </Button>
          </Card>
        </Grid>
      </Grid>
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid size={{ xs: 12, md: 6 }}>
          <TopPerformersList
            title="Most popular plans"
            items={data.topPerformers.topPlans.slice(0, 5).map((plan) => ({
              id: plan.planId,
              name: plan.planName,
              primaryMetric: money(plan.revenue),
              secondaryMetric: plan.purchaseCount,
            }))}
            secondaryLabel="Purchases"
          />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <TopPerformersList
            title="Top players"
            items={data.topPerformers.topPlayers.slice(0, 5).map((player) => ({
              id: player.playerId,
              name: player.playerName,
              primaryMetric: money(player.totalSpent),
              secondaryMetric: player.totalSessions,
            }))}
            secondaryLabel="Sessions"
          />
        </Grid>
      </Grid>
      <Alert
        icon={<SettingsOutlined />}
        severity="info"
        action={
          <Button component={RouterLink} to="/settings" size="small">
            Open configuration
          </Button>
        }
      >
        Keep every location aligned. Manage venue defaults, pricing policies, and overrides from
        Configuration.
      </Alert>
    </PageShell>
  );
}
