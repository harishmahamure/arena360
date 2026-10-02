import { ErrorPanel, PageHeader, PageShell } from '@gaming-cafe/ui';
import { Download, Refresh } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  Chip,
  Divider,
  Grid,
  LinearProgress,
  Skeleton,
  Slider,
  Stack,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useLocation } from 'react-router-dom';
import {
  type AnalyticsDashboard,
  analyticsDashboards,
} from '../../../constants/analyticsDashboards';
import { StatsDateRangeToolbar } from '../../../containers/stats/StatsDateRangeToolbar';
import { useStatsDateRange } from '../../../hooks/useStatsDateRange';
import { type BusinessReport, getBusinessReport } from '../../../services/stats/business';
import {
  Bars,
  DataTable,
  Heatmap,
  Metrics,
  RevenueTrend,
  Section,
  TrackingGap,
} from './AnalyticsComponents';
import {
  type CapacityAssumptions,
  capacityModel,
  defaultAssumptions,
  forecast,
  money,
  number,
  percent,
  pricingScenario,
  ratio,
  sum,
  summarize,
  weekdays,
} from './analyticsModel';

function Assumptions({
  value,
  onChange,
}: {
  value: CapacityAssumptions;
  onChange: (value: CapacityAssumptions) => void;
}) {
  const update = (key: keyof CapacityAssumptions, raw: string) => {
    const n = Number(raw);
    if (!Number.isFinite(n)) return;
    const next = { ...value, [key]: n };
    if (
      next.openHour >= 0 &&
      next.openHour < next.closeHour &&
      next.closeHour <= 24 &&
      Number.isInteger(next.openHour) &&
      Number.isInteger(next.closeHour) &&
      next.hourlyRate >= 0 &&
      next.hourlyRate <= 100000 &&
      next.capturePercent >= 0 &&
      next.capturePercent <= 100
    )
      onChange(next);
  };
  return (
    <Section
      title="Planning assumptions"
      note="Estimates use today's station inventory across the selected dates. Operating hours and rates below are assumptions, not historical schedules or live pricing. Costs, demand limits, and downtime are not deducted."
    >
      <Grid container spacing={2}>
        {(
          [
            ['openHour', 'Open hour (IST)', 0, 23],
            ['closeHour', 'Close hour (24 = midnight)', 1, 24],
            ['hourlyRate', 'Potential rate / hour (INR)', 0, 100000],
            ['capturePercent', 'Unused capacity to fill (%)', 0, 100],
          ] as const
        ).map(([key, label, min, max]) => (
          <Grid key={key} size={{ xs: 12, sm: 6 }}>
            <TextField
              fullWidth
              size="small"
              type="number"
              label={label}
              value={value[key]}
              onChange={(e) => update(key, e.target.value)}
              slotProps={{ htmlInput: { min, max, step: 1 } }}
            />
          </Grid>
        ))}
      </Grid>
    </Section>
  );
}

function Retention({ report }: { report: BusinessReport }) {
  const [segment, setSegment] = useState('visitors');
  const c = report.customers;
  const rows = report.customerDetails.filter((r) =>
    segment === 'risk'
      ? r.daysAbsent >= 30 && r.daysAbsent < 90
      : segment === 'new'
        ? r.visits > 0 && r.firstVisit >= report.period.startDate
        : segment === 'repeat'
          ? r.visits > 0 && r.firstVisit < report.period.startDate
          : r.visits > 0,
  );
  return (
    <Stack gap={3}>
      <Metrics
        items={[
          {
            label: 'New visitors',
            value: number(c.newVisitors),
            hint: 'First recorded session starts in this period',
          },
          {
            label: 'Repeat visitors',
            value: number(c.repeatVisitors),
            hint: 'Visited before this period and returned',
          },
          {
            label: 'Period retention',
            value: percent(ratio(c.retainedVisitors, c.priorVisitors)),
            hint: `${c.retainedVisitors} of ${c.priorVisitors} prior-period visitors returned`,
          },
          {
            label: 'At-risk customers',
            value: number(c.atRisk),
            hint: 'No visit for 30–89 days at period end; rule-based, not a churn prediction',
          },
        ]}
      />
      <Section title="Visit frequency">
        <Bars
          title="Customers"
          rows={[
            { label: 'New', value: c.newVisitors },
            { label: 'Repeat', value: c.repeatVisitors },
            { label: '3+ sessions', value: c.frequentVisitors },
          ]}
        />
      </Section>
      <Section
        title="Customer detail"
        note="Most recent 500 customers with recorded sessions. Summary metrics cover all customers; these segments filter only this detail list. Staff allowance wallets are excluded."
      >
        <ToggleButtonGroup
          value={segment}
          exclusive
          size="small"
          aria-label="Customer segment"
          onChange={(_, v) => {
            if (v) setSegment(v);
          }}
          sx={{ mb: 2, flexWrap: 'wrap' }}
        >
          {[
            ['visitors', 'All visitors'],
            ['new', 'New'],
            ['repeat', 'Repeat'],
            ['risk', 'At risk'],
          ].map(([id, label]) => (
            <ToggleButton key={id} value={id ?? ''}>
              {label}
            </ToggleButton>
          ))}
        </ToggleButtonGroup>
        <DataTable
          title="Customer retention detail"
          rows={rows}
          columns={[
            { label: 'Customer', cell: (r) => r.name },
            { label: 'Sessions', cell: (r) => r.visits },
            { label: 'First visit', cell: (r) => r.firstVisit },
            { label: 'Last visit', cell: (r) => r.lastVisit },
            { label: 'Period spend', cell: (r) => money(r.spend) },
            { label: 'Days absent', cell: (r) => r.daysAbsent },
          ]}
        />
      </Section>
    </Stack>
  );
}
function Stations({ report }: { report: BusinessReport }) {
  const [metric, setMetric] = useState<'hours' | 'sessions'>('hours');
  const totals = summarize(report);
  return (
    <Stack gap={3}>
      <Metrics
        items={[
          { label: 'Stations in current inventory', value: number(report.stations.length) },
          {
            label: 'Calendar utilization',
            value: percent(
              ratio(
                sum(report.stations, (r) => r.hours),
                totals.elapsedHours * report.stations.length,
              ),
            ),
            hint: 'All elapsed hours × current inventory; not opening-hours adjusted',
          },
          {
            label: 'Plan sales / played hour',
            value: money(ratio(totals.planRevenue, totals.hours)),
            hint: 'Sales-to-usage ratio, not a realized hourly price',
          },
        ]}
      />
      <Section
        title="Station activity"
        note="Hover a bar for detail. Recorded occupied hours are clipped to the selected period."
      >
        <ToggleButtonGroup
          size="small"
          exclusive
          value={metric}
          onChange={(_, v) => {
            if (v) setMetric(v);
          }}
          aria-label="Station metric"
        >
          <ToggleButton value="hours">Hours</ToggleButton>
          <ToggleButton value="sessions">Sessions</ToggleButton>
        </ToggleButtonGroup>
        <Bars
          title={metric === 'hours' ? 'Occupied hours' : 'Sessions started'}
          rows={[...report.stations]
            .sort((a, b) => b[metric] - a[metric])
            .map((r) => ({ label: r.name, value: r[metric] }))}
        />
      </Section>
      <Section
        title="Station economics"
        note="Allocated sales distribute period plan sales in proportion to played hours. They are an estimate, not direct station revenue. Status is current, not historical downtime."
      >
        <DataTable
          title="Station economics"
          rows={report.stations}
          columns={[
            { label: 'Station', cell: (r) => r.name },
            { label: 'Area', cell: (r) => r.location },
            { label: 'Current status', cell: (r) => r.status },
            { label: 'Hours', cell: (r) => number(r.hours) },
            { label: 'Sessions', cell: (r) => r.sessions },
            {
              label: 'Allocated plan sales',
              cell: (r) =>
                money(totals.hours > 0 ? (totals.planRevenue * r.hours) / totals.hours : null),
            },
          ]}
        />
      </Section>
      <TrackingGap
        title="Downtime measurement"
        items={[
          'Station offline/online and maintenance intervals are required to distinguish an idle station from a broken one. Current status cannot reconstruct historical downtime.',
        ]}
      />
    </Stack>
  );
}
function Pricing({
  report,
  assumptions,
}: {
  report: BusinessReport;
  assumptions: CapacityAssumptions;
}) {
  const [price, setPrice] = useState(10);
  const [demand, setDemand] = useState(0);
  const model = capacityModel(report, assumptions);
  const estimate = pricingScenario(
    model.used,
    model.available,
    assumptions.hourlyRate,
    price,
    demand,
  );
  return (
    <Stack gap={3}>
      <Alert severity="info">
        This is a what-if model. Price sensitivity has not been measured. No live prices change from
        this dashboard.
      </Alert>
      <Section title="Price and demand scenario">
        <Typography>Price change: {price}%</Typography>
        <Slider
          aria-label="Price change percent"
          value={price}
          min={-50}
          max={100}
          step={5}
          valueLabelDisplay="auto"
          onChange={(_, v) => setPrice(v as number)}
        />
        <Typography>Assumed occupied-hours change: {demand}%</Typography>
        <Slider
          aria-label="Demand change percent"
          value={demand}
          min={-100}
          max={100}
          step={5}
          valueLabelDisplay="auto"
          onChange={(_, v) => setDemand(v as number)}
        />
        <Metrics
          items={[
            { label: 'Baseline at assumed rate', value: money(estimate.baseline) },
            { label: 'Scenario gross revenue', value: money(estimate.projected) },
            {
              label: 'Scenario difference',
              value: money(estimate.delta),
              hint: 'Before costs; demand assumption is unvalidated',
            },
            {
              label: 'Scenario occupied hours',
              value: number(estimate.projectedHours),
              hint: 'Capped at estimated capacity',
            },
          ]}
        />
      </Section>
      <Section title="Demand by slot">
        <Heatmap model={model} />
      </Section>
      <TrackingGap
        title="Before changing pricing"
        items={[
          'Measure comparable slot prices, exposure, availability, and bookings. Use controlled tests to estimate price sensitivity; demand differences across hours alone do not establish it.',
        ]}
      />
    </Stack>
  );
}
function ForecastView({ report }: { report: BusinessReport }) {
  const predicted = forecast(report);
  const totals = summarize(report);
  const repeat = ratio(report.customers.repeatVisitors, report.customers.visitors);
  const attach = ratio(report.pos.attachedCustomers, report.pos.visitingCustomers);
  const recommendations = [
    {
      title: 'Retention',
      evidence: `${report.customers.atRisk} customers have been absent 30–89 days.`,
      action:
        report.customers.atRisk > 0
          ? 'Review the at-risk segment and test a win-back offer.'
          : 'Keep monitoring return visits.',
    },
    {
      title: 'Secondary spend',
      evidence: `Visitor POS attach rate: ${percent(attach)}.`,
      action:
        attach !== null && attach < 0.3
          ? 'Test a session + snack bundle, then compare attach rate.'
          : 'Review top products and their stock availability.',
    },
    {
      title: 'Repeat visits',
      evidence: `Repeat visitor share: ${percent(repeat)}.`,
      action:
        'Track this alongside new visitors; a higher repeat share alone may also mean fewer new visitors.',
    },
  ];
  return (
    <Stack gap={3}>
      <Alert severity="info">
        Baseline estimates use the average of complete matching weekdays in your selected period,
        including zero-sales days. They do not model holidays, promotions, seasonality, or future
        growth.
      </Alert>
      <Section
        title="Next seven days after the selected period"
        note="Historical low/high are observed weekday extremes, not confidence intervals. At least 14 complete days are required."
      >
        {predicted.length ? (
          <>
            <Bars
              title="Expected sales (weekday baseline)"
              format={money}
              rows={predicted.map((r) => ({ label: r.date, value: r.revenue }))}
            />
            <DataTable
              title="Weekday forecast"
              rows={predicted}
              columns={[
                { label: 'Date', cell: (r) => r.date },
                { label: 'Baseline sales', cell: (r) => money(r.revenue) },
                { label: 'Historical low', cell: (r) => money(r.low) },
                { label: 'Historical high', cell: (r) => money(r.high) },
                { label: 'Sample days', cell: (r) => r.samples },
              ]}
            />
          </>
        ) : (
          <Alert severity="warning">
            Select at least 14 complete days to produce a weekday baseline. No forecast is available
            for this range.
          </Alert>
        )}
      </Section>
      <Section
        title="Recommendations to investigate"
        note={`Transparent rules based on ${totals.sessions} session starts and ${totals.transactions} sales, not an automated decision model.`}
      >
        <Stack spacing={2}>
          {recommendations.map((r) => (
            <Box key={r.title}>
              <Typography fontWeight={650}>{r.title}</Typography>
              <Typography variant="body2">{r.evidence}</Typography>
              <Typography variant="body2" color="text.secondary">
                {r.action}
              </Typography>
            </Box>
          ))}
        </Stack>
      </Section>
    </Stack>
  );
}

function DashboardBody({
  id,
  report,
  compare,
  assumptions,
}: {
  id: AnalyticsDashboard;
  report: BusinessReport;
  compare: boolean;
  assumptions: CapacityAssumptions;
}) {
  const totals = summarize(report);
  const capacity = capacityModel(report, assumptions);
  if (capacity.inconsistent && (id === 'opportunity' || id === 'pricing')) {
    return (
      <Alert severity="warning">
        Recorded usage exceeds the estimated inventory capacity in one or more slots. Review
        overlapping sessions and historical station counts in Busy Hours &amp; Capacity before using
        revenue or pricing scenarios. Scenarios are unavailable for this range.
      </Alert>
    );
  }
  switch (id) {
    case 'executive':
      return (
        <Stack gap={3}>
          <Metrics
            items={[
              {
                label: 'Sales revenue',
                value: money(totals.revenue),
                hint: compare
                  ? `${percent(totals.growth)} vs previous equal calendar period (— when previous sales are zero)`
                  : 'Completed and credit sales',
              },
              {
                label: 'Sessions started',
                value: number(totals.sessions),
                hint: `${number(totals.hours)} occupied hours, including sessions crossing the boundary`,
              },
              {
                label: 'Average ticket',
                value: money(totals.averageTicket),
                hint: `Across ${totals.transactions} sales`,
              },
              {
                label: 'Repeat visitor share',
                value: percent(ratio(report.customers.repeatVisitors, report.customers.visitors)),
                hint: `${report.customers.repeatVisitors} of ${report.customers.visitors} visitors`,
              },
              {
                label: 'Calendar utilization',
                value: percent(ratio(totals.hours, totals.elapsedHours * report.stations.length)),
                hint: 'Estimated using current station inventory over all elapsed hours',
              },
              {
                label: 'Active prepaid holders now',
                value: number(report.wallets.holders),
                hint: 'Current wallet snapshot; not subscription members at the historical date',
              },
            ]}
          />
          <RevenueTrend report={report} compare={compare} />
        </Stack>
      );
    case 'locations': {
      const groups = new Map<
        string,
        { name: string; stations: number; hours: number; sessions: number }
      >();
      for (const station of report.stations) {
        const row = groups.get(station.location) ?? {
          name: station.location,
          stations: 0,
          hours: 0,
          sessions: 0,
        };
        row.stations++;
        row.hours += station.hours;
        row.sessions += station.sessions;
        groups.set(row.name, row);
      }
      const rows = [...groups.values()].sort((a, b) => b.hours - a.hours);
      return (
        <Stack gap={3}>
          <Alert severity="info">
            Current scope: the original venue ledger. These groups are device area labels, not
            independently scoped branches.
          </Alert>
          <Section title="Activity by station area">
            <Bars
              title="Occupied hours"
              rows={rows.map((r) => ({ label: r.name, value: r.hours }))}
            />
            <DataTable
              title="Location activity"
              rows={rows}
              columns={[
                { label: 'Area', cell: (r) => r.name },
                { label: 'Stations', cell: (r) => r.stations },
                { label: 'Sessions', cell: (r) => r.sessions },
                { label: 'Hours', cell: (r) => number(r.hours) },
                {
                  label: 'Calendar utilization',
                  cell: (r) => percent(ratio(r.hours, r.stations * totals.elapsedHours)),
                },
              ]}
            />
          </Section>
          <TrackingGap
            title="Branch comparisons"
            items={[
              'Branch revenue, revenue growth, and average ticket require an immutable venue/location ID on each sale and session. Inventory storage locations and free-text device areas cannot safely provide that attribution.',
            ]}
          />
        </Stack>
      );
    }
    case 'capacity':
      return (
        <Stack gap={3}>
          <Metrics
            items={[
              { label: 'Occupied operating hours', value: number(capacity.used) },
              { label: 'Estimated station-hours', value: number(capacity.available) },
              { label: 'Estimated occupancy', value: percent(capacity.occupancy) },
              { label: 'Estimated unused hours', value: number(capacity.unused) },
            ]}
          />
          <Section title="Weekday × hour occupancy">
            <Heatmap model={capacity} />
          </Section>
          <TrackingGap
            title="Unserved demand"
            items={[
              'Waiting-list entries, rejected bookings, and their timestamps are not captured. Occupancy alone cannot tell you how many customers were turned away.',
            ]}
          />
        </Stack>
      );
    case 'opportunity':
      return (
        <Stack gap={3}>
          <Alert severity="warning">
            Unused capacity is not proven lost revenue. The gross ceiling assumes every unused hour
            could sell; the scenario discounts that by your chosen fill target.
          </Alert>
          <Metrics
            items={[
              { label: 'Unused station-hours', value: number(capacity.unused) },
              {
                label: 'Gross capacity ceiling',
                value: money(capacity.ceiling),
                hint: 'Unused hours × assumed hourly rate',
              },
              {
                label: 'Target additional hours',
                value: number((capacity.unused * assumptions.capturePercent) / 100),
              },
              {
                label: 'Scenario gross opportunity',
                value: money(capacity.opportunity),
                hint: `At ${assumptions.capturePercent}% fill; excludes costs`,
              },
            ]}
          />
          <Section title="Opportunity by weekday">
            <Bars
              title="Scenario revenue"
              format={money}
              rows={weekdays.map((label, d) => ({
                label,
                value:
                  (sum(
                    capacity.cells.filter((c) => c.weekday === d),
                    (c) => Math.max(0, c.available - c.used),
                  ) *
                    assumptions.hourlyRate *
                    assumptions.capturePercent) /
                  100,
              }))}
            />
          </Section>
        </Stack>
      );
    case 'pricing':
      return <Pricing report={report} assumptions={assumptions} />;
    case 'retention':
      return <Retention report={report} />;
    case 'memberships':
      return (
        <Stack gap={3}>
          <Alert severity="info">
            The product currently stores prepaid plans and wallets, not recurring subscription
            memberships. Active holders are a current snapshot. Repeated plan purchases are a
            repurchase proxy, not renewal or recurring revenue.
          </Alert>
          <Metrics
            items={[
              { label: 'Active prepaid holders now', value: number(report.wallets.holders) },
              { label: 'Active wallets now', value: number(report.wallets.activeWallets) },
              { label: 'Wallets expiring in 7 days', value: number(report.wallets.expiring7Days) },
              {
                label: 'Remaining prepaid hours now',
                value: number(report.wallets.remainingHours),
              },
            ]}
          />
          <Section title="Plan sales in selected period">
            <Bars
              title="Plan sales"
              format={money}
              rows={report.plans.map((r) => ({ label: r.name, value: r.revenue }))}
            />
            <DataTable
              title="Plan repurchases"
              rows={report.plans}
              columns={[
                { label: 'Plan', cell: (r) => r.name },
                { label: 'Sales', cell: (r) => money(r.revenue) },
                { label: 'Purchases', cell: (r) => r.purchases },
                { label: 'Buyers', cell: (r) => r.buyers },
                { label: 'Repeat buyers in period', cell: (r) => r.repeatBuyers },
                {
                  label: 'Repurchase share',
                  cell: (r) => percent(ratio(r.repeatBuyers, r.buyers)),
                },
                { label: 'Catalog hours sold', cell: (r) => number(r.hoursSold) },
              ]}
            />
          </Section>
          <Alert severity="info">
            Catalog hours sold use the current plan's time credits. Historical entitlement snapshots
            and renewal events are needed for precise membership consumption and renewal rates.
          </Alert>
        </Stack>
      );
    case 'stations':
      return <Stations report={report} />;
    case 'pos':
      return (
        <Stack gap={3}>
          <Metrics
            items={[
              {
                label: 'F&B / POS sales',
                value: money(report.pos.revenue),
                hint: `${report.pos.transactions} transactions`,
              },
              {
                label: 'POS sales / session start',
                value: money(ratio(report.pos.revenue, totals.sessions)),
                hint: 'Aggregate ratio, not individually attributed session spend',
              },
              {
                label: 'Visitor POS attach rate',
                value: percent(ratio(report.pos.attachedCustomers, report.pos.visitingCustomers)),
                hint: 'Visitors who also bought POS in the period; purchases need not occur during the same session',
              },
              {
                label: 'Attached visitors',
                value: number(report.pos.attachedCustomers),
                hint: `Of ${report.pos.visitingCustomers} session visitors`,
              },
            ]}
          />
          <Section
            title="Top products"
            note="Top 100 by line-item sales. Line-item totals can differ from transaction totals when legacy adjustments are present."
          >
            <Bars
              title="Product sales"
              format={money}
              rows={report.products.slice(0, 15).map((r) => ({ label: r.name, value: r.revenue }))}
            />
            <DataTable
              title="Product performance"
              rows={report.products}
              columns={[
                { label: 'Product', cell: (r) => r.name },
                { label: 'Units', cell: (r) => r.quantity },
                { label: 'Line sales', cell: (r) => money(r.revenue) },
                { label: 'Transactions', cell: (r) => r.transactions },
              ]}
            />
          </Section>
        </Stack>
      );
    case 'staff':
      return (
        <Stack gap={3}>
          <Alert severity="info">
            Sales and session starts are attributed to the recorded creator. Shift hours are clipped
            to the range; a zero-hour denominator is shown as unavailable. These ratios do not
            adjust for demand, role, or staffing mix.
          </Alert>
          <Section title="Staff sales">
            <Bars
              title="Attributed sales"
              format={money}
              rows={report.staff.map((r) => ({ label: r.name, value: r.revenue }))}
            />
            <DataTable
              title="Staff operations"
              rows={report.staff}
              columns={[
                { label: 'Staff', cell: (r) => r.name },
                { label: 'Shift hours', cell: (r) => number(r.shiftHours) },
                { label: 'Sales', cell: (r) => money(r.revenue) },
                { label: 'Sales / shift-hour', cell: (r) => money(ratio(r.revenue, r.shiftHours)) },
                { label: 'Transactions', cell: (r) => r.transactions },
                { label: 'Sessions started', cell: (r) => r.sessions },
              ]}
            />
          </Section>
          <TrackingGap
            title="Operational controls"
            items={[
              'Override reasons, authorized discounts, and staffing by slot require structured audit events and shift-overlap reporting. Missing attribution must not be interpreted as zero leakage.',
            ]}
          />
        </Stack>
      );
    case 'forecast':
      return <ForecastView report={report} />;
  }
}

function downloadDaily(report: BusinessReport) {
  const rows = summarize(report).daily;
  const csv = [
    'date_IST,sales_INR,plan_sales_INR,pos_sales_INR,transactions',
    ...rows.map((r) => [r.date, r.revenue, r.planRevenue, r.posRevenue, r.transactions].join(',')),
  ].join('\r\n');
  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `arena360-sales-${report.period.startDate}-${report.period.endDate}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}
export default function AnalyticsPage({ dashboard }: { dashboard: AnalyticsDashboard }) {
  const location = useLocation();
  const definition = analyticsDashboards.find((d) => d.id === dashboard) ?? analyticsDashboards[0];
  const range = useStatsDateRange('last30');
  const [assumptions, setAssumptions] = useState(defaultAssumptions);
  const start = range.apiFilters.startDate.slice(0, 10);
  const end = range.apiFilters.endDate.slice(0, 10);
  const { data, isLoading, isFetching, error, refetch } = useQuery({
    queryKey: ['businessAnalytics', start, end],
    queryFn: () => getBusinessReport(start, end),
    staleTime: 30000,
    retry: 1,
  });
  const assumptionsRelevant = ['capacity', 'opportunity', 'pricing'].includes(definition.id);
  const body = useMemo(
    () =>
      data ? (
        <DashboardBody
          id={definition.id}
          report={data}
          compare={range.appliedCompare}
          assumptions={assumptions}
        />
      ) : null,
    [data, definition.id, range.appliedCompare, assumptions],
  );
  return (
    <PageShell
      header={
        <PageHeader
          title={definition.title}
          description={definition.action}
          breadcrumbs={[
            { label: 'Business dashboard', to: `/analytics${location.search}` },
            { label: definition.title },
          ]}
        />
      }
    >
      <Stack gap={3}>
        <Card sx={{ p: 2 }}>
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
          <Typography variant="caption" color="text.secondary">
            IST calendar dates · up to 366 days · choose both dates, then Apply
          </Typography>
        </Card>
        <Stack gap={3} sx={{ minWidth: 0 }}>
          <Stack direction="row" gap={2} justifyContent="space-between" flexWrap="wrap">
            <Chip size="small" label={definition.coverage} variant="outlined" />
            <Stack direction="row" gap={1}>
              <Button
                size="small"
                startIcon={<Refresh />}
                disabled={isFetching}
                onClick={() => void refetch()}
              >
                Refresh
              </Button>
              <Button
                size="small"
                startIcon={<Download />}
                disabled={!data || isFetching}
                onClick={() => data && downloadDaily(data)}
              >
                Sales CSV
              </Button>
            </Stack>
          </Stack>
          {isFetching && <LinearProgress aria-label="Loading analytics" />}
          {isLoading ? (
            <Skeleton variant="rounded" height={360} />
          ) : error || !data ? (
            <ErrorPanel
              message="Analytics could not load. Check the date range and ensure the business projections have finished syncing."
              onRetry={() => void refetch()}
            />
          ) : (
            <>
              <Typography variant="caption" color="text.secondary">
                {data.period.startDate} — {data.period.endDate} · Original venue · Report generated{' '}
                {new Date(data.generatedAt).toLocaleString('en-IN', { timeZone: 'Asia/Kolkata' })}{' '}
                IST · May lag recent activity; cached for 30 seconds.
              </Typography>
              {assumptionsRelevant && <Assumptions value={assumptions} onChange={setAssumptions} />}
              {body}
              <Divider />
              <Typography variant="caption" color="text.secondary">
                Revenue counts completed and credit sales once; refunded, failed, deleted, and
                pending transactions are excluded. Session metrics use recorded intervals. Missing
                tracking and modeled values are identified in each report.
              </Typography>
            </>
          )}
        </Stack>
      </Stack>
    </PageShell>
  );
}
