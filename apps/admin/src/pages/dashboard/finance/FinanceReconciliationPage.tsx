import { ErrorPanel, PageHeader, PageShell } from '@gaming-cafe/ui';
import {
  AccountBalanceWallet,
  CheckCircle,
  HourglassEmpty,
  LockOpen,
  PointOfSale,
} from '@mui/icons-material';
import { Button, Grid, Skeleton } from '@mui/material';
import { Link as RouterLink } from 'react-router-dom';
import { StatCard, type StatTone } from '../../../containers/stats/StatCard';
import { StatsDateRangeToolbar } from '../../../containers/stats/StatsDateRangeToolbar';
import { useFinanceReconciliationStats } from '../../../hooks/useFinanceReconciliationStats';
import { useStatsDateRange } from '../../../hooks/useStatsDateRange';
import { calculatePeriodChange } from '../../../services/stats/statsHelpers';

export default function FinanceReconciliationPage() {
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
  const { data, isLoading, error, refetch } = useFinanceReconciliationStats(apiFilters);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const change = (current: number, previous?: number | null) =>
    appliedCompare ? calculatePeriodChange(current, previous ?? undefined) : undefined;

  if (isLoading) {
    return (
      <PageShell>
        <Skeleton variant="text" width={240} height={40} sx={{ mb: 2 }} />
        <Grid container spacing={3}>
          {['a', 'b', 'c', 'd', 'e'].map((id) => (
            <Grid key={id} size={{ xs: 12, sm: 6, md: 4 }}>
              <Skeleton variant="rounded" height={140} />
            </Grid>
          ))}
        </Grid>
      </PageShell>
    );
  }

  if (error || !data) {
    return (
      <PageShell>
        <ErrorPanel
          message="Failed to load reconciliation statistics."
          onRetry={() => void refetch()}
        />
      </PageShell>
    );
  }

  const current = data.metrics.current;
  const previous = data.metrics.previous;
  const description =
    appliedCompare && data.period.previousLabel
      ? `${data.period.label} · compared to ${data.period.previousLabel}`
      : data.period.label;

  const stats: {
    title: string;
    value: string;
    change?: { value: string | number; positive: boolean };
    icon: typeof PointOfSale;
    tone: StatTone;
    subtitle?: string;
  }[] = [
    {
      title: 'Open registers',
      value: current.openCount.toLocaleString(),
      change: change(current.openCount, previous?.openCount),
      icon: LockOpen,
      tone: 'warning',
    },
    {
      title: 'Closed (pending recon)',
      value: current.pendingReconcileCount.toLocaleString(),
      change: change(current.pendingReconcileCount, previous?.pendingReconcileCount),
      icon: HourglassEmpty,
      tone: 'info',
    },
    {
      title: 'Reconciled',
      value: current.reconciledCount.toLocaleString(),
      change: change(current.reconciledCount, previous?.reconciledCount),
      icon: CheckCircle,
      tone: 'success',
    },
    {
      title: 'Closed registers',
      value: current.closedCount.toLocaleString(),
      change: change(current.closedCount, previous?.closedCount),
      icon: PointOfSale,
      tone: 'primary',
    },
    {
      title: 'Total deposited',
      value: formatCurrency(current.totalDeposited),
      change: change(current.totalDeposited, previous?.totalDeposited),
      icon: AccountBalanceWallet,
      tone: 'success',
      subtitle: 'Approved deposits in period',
    },
  ];

  return (
    <PageShell
      header={<PageHeader title="Reconciliation" description={description} />}
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
      <Grid container spacing={3} sx={{ mb: 3 }}>
        {stats.map((stat) => (
          <Grid key={stat.title} size={{ xs: 12, sm: 6, md: 4 }}>
            <StatCard
              title={stat.title}
              value={stat.value}
              subtitle={stat.subtitle}
              change={stat.change}
              tone={stat.tone}
              icon={<stat.icon sx={{ fontSize: 24 }} />}
            />
          </Grid>
        ))}
      </Grid>
      <Button component={RouterLink} to="/cash-registers" variant="outlined">
        View cash registers
      </Button>
    </PageShell>
  );
}
