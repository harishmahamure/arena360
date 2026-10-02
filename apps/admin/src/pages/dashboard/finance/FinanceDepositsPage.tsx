import { ErrorPanel, PageHeader, PageShell } from '@gaming-cafe/ui';
import { AccountBalance, CheckCircle, Home, HourglassEmpty, MoneyOff } from '@mui/icons-material';
import { Button, Grid, Skeleton } from '@mui/material';
import { Link as RouterLink } from 'react-router-dom';
import { StatCard, type StatTone } from '../../../containers/stats/StatCard';
import { StatsDateRangeToolbar } from '../../../containers/stats/StatsDateRangeToolbar';
import { useFinanceDepositStats } from '../../../hooks/useFinanceDepositStats';
import { useStatsDateRange } from '../../../hooks/useStatsDateRange';
import { calculatePeriodChange } from '../../../services/stats/statsHelpers';

export default function FinanceDepositsPage() {
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
  const { data, isLoading, error, refetch } = useFinanceDepositStats(apiFilters);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const change = (current: number, previous?: number | null) =>
    appliedCompare ? calculatePeriodChange(current, previous ?? undefined) : undefined;

  if (isLoading) {
    return (
      <PageShell>
        <Skeleton variant="text" width={240} height={40} sx={{ mb: 2 }} />
        <Grid container spacing={3}>
          {['a', 'b', 'c', 'd', 'e', 'f'].map((id) => (
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
        <ErrorPanel message="Failed to load deposit statistics." onRetry={() => void refetch()} />
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
    icon: typeof HourglassEmpty;
    tone: StatTone;
    subtitle?: string;
  }[] = [
    {
      title: 'Pending deposits',
      value: current.pendingCount.toLocaleString(),
      change: change(current.pendingCount, previous?.pendingCount),
      icon: HourglassEmpty,
      tone: 'warning',
      subtitle: formatCurrency(current.pendingAmount),
    },
    {
      title: 'Approved deposits',
      value: current.approvedCount.toLocaleString(),
      change: change(current.approvedCount, previous?.approvedCount),
      icon: CheckCircle,
      tone: 'success',
      subtitle: formatCurrency(current.approvedAmount),
    },
    {
      title: 'Rejected deposits',
      value: current.rejectedCount.toLocaleString(),
      change: change(current.rejectedCount, previous?.rejectedCount),
      icon: MoneyOff,
      tone: 'error',
      subtitle: formatCurrency(current.rejectedAmount),
    },
    {
      title: 'Bank deposits',
      value: formatCurrency(current.bankAmount),
      change: change(current.bankAmount, previous?.bankAmount),
      icon: AccountBalance,
      tone: 'primary',
      subtitle: 'Approved → bank',
    },
    {
      title: 'Home deposits',
      value: formatCurrency(current.homeAmount),
      change: change(current.homeAmount, previous?.homeAmount),
      icon: Home,
      tone: 'info',
      subtitle: 'Approved → home',
    },
  ];

  return (
    <PageShell
      header={<PageHeader title="Cash deposits" description={description} />}
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
      <Button component={RouterLink} to="/cash-deposits" variant="outlined">
        View deposit list
      </Button>
    </PageShell>
  );
}
