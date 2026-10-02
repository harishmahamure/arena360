import { type Column, ErrorPanel, ListPage, PageHeader, PageShell } from '@gaming-cafe/ui';
import { Balance, TrendingDown, TrendingFlat, TrendingUp, Visibility } from '@mui/icons-material';
import { Button, Chip, Grid, Skeleton, Typography } from '@mui/material';
import { useMemo } from 'react';
import { Link as RouterLink, useNavigate } from 'react-router-dom';
import { StatCard, type StatTone } from '../../../containers/stats/StatCard';
import { StatsDateRangeToolbar } from '../../../containers/stats/StatsDateRangeToolbar';
import { useFinanceVarianceStats } from '../../../hooks/useFinanceVarianceStats';
import { useStatsDateRange } from '../../../hooks/useStatsDateRange';
import { calculatePeriodChange } from '../../../services/stats/statsHelpers';
import { formatDisplayDateTime } from '../../../utils/date';

type VarianceRow = {
  id: string;
  shiftId: string;
  status: string;
  variance: number;
  closingBalance?: number | null;
  expectedClosing?: number | null;
  updatedAt: string;
};

export default function FinanceVariancePage() {
  const navigate = useNavigate();
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
  const { data, isLoading, error, refetch } = useFinanceVarianceStats(apiFilters);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const change = (current: number, previous?: number | null) =>
    appliedCompare ? calculatePeriodChange(current, previous ?? undefined) : undefined;

  const rows: VarianceRow[] = useMemo(() => data?.registers ?? [], [data]);

  const columns: Column<VarianceRow>[] = [
    {
      id: 'updatedAt',
      label: 'Closed',
      minWidth: 140,
      format: (v) => formatDisplayDateTime(v as string),
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 100,
      format: (v) => (
        <Chip size="small" label={String(v)} color={v === 'reconciled' ? 'info' : 'default'} />
      ),
    },
    {
      id: 'expectedClosing',
      label: 'Expected',
      minWidth: 110,
      format: (v) => (v == null ? '—' : formatCurrency(v as number)),
    },
    {
      id: 'closingBalance',
      label: 'Closing',
      minWidth: 110,
      format: (v) => (v == null ? '—' : formatCurrency(v as number)),
    },
    {
      id: 'variance',
      label: 'Variance',
      minWidth: 110,
      format: (v) => {
        const n = v as number;
        return (
          <Typography
            fontWeight={600}
            color={n > 0 ? 'success.main' : n < 0 ? 'error.main' : 'text.primary'}
          >
            {formatCurrency(n)}
          </Typography>
        );
      },
    },
  ];

  if (isLoading) {
    return (
      <PageShell>
        <Skeleton variant="text" width={240} height={40} sx={{ mb: 2 }} />
        <Grid container spacing={3}>
          {['a', 'b', 'c', 'd'].map((id) => (
            <Grid key={id} size={{ xs: 12, sm: 6, md: 3 }}>
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
        <ErrorPanel message="Failed to load variance statistics." onRetry={() => void refetch()} />
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
    icon: typeof Balance;
    tone: StatTone;
    subtitle?: string;
  }[] = [
    {
      title: 'Total variance',
      value: formatCurrency(current.totalVariance),
      change: change(current.totalVariance, previous?.totalVariance),
      icon: Balance,
      tone: current.totalVariance >= 0 ? 'success' : 'error',
      subtitle: `${current.registerCount} closed registers`,
    },
    {
      title: 'Average variance',
      value: formatCurrency(current.averageVariance),
      change: change(current.averageVariance, previous?.averageVariance),
      icon: TrendingFlat,
      tone: 'info',
    },
    {
      title: 'Over (positive)',
      value: current.overCount.toLocaleString(),
      change: change(current.overCount, previous?.overCount),
      icon: TrendingUp,
      tone: 'success',
    },
    {
      title: 'Short (negative)',
      value: current.shortCount.toLocaleString(),
      change: change(current.shortCount, previous?.shortCount),
      icon: TrendingDown,
      tone: 'error',
    },
  ];

  return (
    <>
      <PageShell
        header={<PageHeader title="Variance" description={description} />}
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
            <Grid key={stat.title} size={{ xs: 12, sm: 6, md: 3 }}>
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

        <Button component={RouterLink} to="/cash-registers" variant="outlined" sx={{ mb: 1 }}>
          View cash registers
        </Button>
      </PageShell>

      <ListPage
        title="Registers with variance"
        description="Closed or reconciled registers in the selected period, sorted by absolute variance"
        columns={columns}
        data={rows}
        actions={[
          {
            label: 'View',
            icon: <Visibility fontSize="small" />,
            onClick: (row) => navigate(`/cash-registers/${row.id}`),
          },
        ]}
        isLoading={false}
      />
    </>
  );
}
