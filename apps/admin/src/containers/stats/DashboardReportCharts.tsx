import { ExpandMore } from '@mui/icons-material';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Chip,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import { BarChart } from '@mui/x-charts/BarChart';
import { LineChart } from '@mui/x-charts/LineChart';
import { useMemo, useState } from 'react';
import type {
  DeviceStatsDto,
  RevenueByPaymentMethodDto,
  RevenueTrendDto,
} from '../../services/stats/types';

const money = (value: number | null) =>
  value === null
    ? '—'
    : new Intl.NumberFormat('en-IN', {
        style: 'currency',
        currency: 'INR',
        maximumFractionDigits: 2,
      }).format(value);
const compact = (value: number) =>
  new Intl.NumberFormat('en-IN', { notation: 'compact' }).format(value);
const dateLabel = (value: string) =>
  new Intl.DateTimeFormat('en-IN', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    timeZone: 'Asia/Kolkata',
  }).format(new Date(value));

const revenueSeries = [
  { id: 'totalRevenue', label: 'Total revenue', color: '#268268' },
  { id: 'cashRevenue', label: 'Cash', color: '#598ab1' },
  { id: 'onlineRevenue', label: 'Online', color: '#bd944d' },
] as const;

export function RevenueReportChart({ rows }: { rows: RevenueTrendDto[] }) {
  const [metric, setMetric] = useState<'revenue' | 'transactions'>('revenue');
  const [visible, setVisible] = useState<string[]>(['totalRevenue']);
  const sorted = useMemo(() => [...rows].sort((a, b) => a.date.localeCompare(b.date)), [rows]);
  const series =
    metric === 'transactions'
      ? [
          {
            id: 'transactions',
            label: 'Transactions',
            color: '#598ab1',
            data: sorted.map((row) => row.transactionCount),
            valueFormatter: (v: number | null) => String(v ?? '—'),
          },
        ]
      : revenueSeries
          .filter((item) => visible.includes(item.id))
          .map((item) => ({
            ...item,
            data: sorted.map((row) => row[item.id]),
            valueFormatter: money,
          }));

  return (
    <Box sx={{ mt: 2, minWidth: 0 }}>
      <Stack direction="row" gap={1} flexWrap="wrap" alignItems="center">
        <ToggleButtonGroup
          exclusive
          size="small"
          value={metric}
          aria-label="Trend metric"
          onChange={(_, value) => {
            if (value) setMetric(value);
          }}
        >
          <ToggleButton value="revenue">Revenue</ToggleButton>
          <ToggleButton value="transactions">Transactions</ToggleButton>
        </ToggleButtonGroup>
        {metric === 'revenue' &&
          revenueSeries.map((item) => (
            <Chip
              key={item.id}
              label={item.label}
              icon={
                <Box
                  component="span"
                  sx={{ width: 8, height: 8, borderRadius: '50%', bgcolor: item.color }}
                />
              }
              component="button"
              type="button"
              aria-pressed={visible.includes(item.id)}
              variant={visible.includes(item.id) ? 'filled' : 'outlined'}
              onClick={() =>
                setVisible((current) =>
                  current.includes(item.id)
                    ? current.length > 1
                      ? current.filter((id) => id !== item.id)
                      : current
                    : [...current, item.id],
                )
              }
              sx={{ borderColor: item.color }}
            />
          ))}
      </Stack>
      {sorted.length ? (
        <LineChart
          title={metric === 'revenue' ? 'Daily revenue in INR' : 'Daily transactions'}
          height={300}
          series={series.map((item) => ({
            ...item,
            curve: 'linear',
            showMark: sorted.length <= 31,
          }))}
          xAxis={[
            {
              scaleType: 'time',
              data: sorted.map((row) => new Date(`${row.date}T00:00:00+05:30`)),
              valueFormatter: (value: Date) => dateLabel(value.toISOString()),
              tickMinStep: 86_400_000,
              tickNumber: 5,
            },
          ]}
          yAxis={[
            {
              min: 0,
              valueFormatter: compact,
              width: 60,
              ...(metric === 'transactions' ? { tickMinStep: 1 } : {}),
            },
          ]}
          grid={{ horizontal: true }}
          hideLegend
          skipAnimation
          margin={{ left: 5, right: 20, top: 20, bottom: 10 }}
          axisHighlight={{ x: 'line' }}
        />
      ) : (
        <Typography color="text.secondary" sx={{ py: 8, textAlign: 'center' }}>
          No revenue activity in this period. Try a wider date range.
        </Typography>
      )}
      <Typography variant="caption" color="text.secondary">
        Hover or touch to inspect reported days (IST). Total includes running tabs and settlements.
      </Typography>
      {sorted.length > 0 && (
        <Accordion disableGutters elevation={0} sx={{ mt: 1, '&:before': { display: 'none' } }}>
          <AccordionSummary expandIcon={<ExpandMore />}>
            <Typography variant="body2">View daily report data</Typography>
          </AccordionSummary>
          <AccordionDetails sx={{ p: 0 }}>
            <TableContainer sx={{ maxHeight: 320 }}>
              <Table size="small" stickyHeader aria-label="Daily revenue report">
                <TableHead>
                  <TableRow>
                    {['Date (IST)', 'Total revenue', 'Cash', 'Online', 'Transactions'].map(
                      (label, index) => (
                        <TableCell key={label} align={index ? 'right' : 'left'}>
                          {label}
                        </TableCell>
                      ),
                    )}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {sorted.map((row) => (
                    <TableRow key={row.date}>
                      <TableCell component="th" scope="row">
                        {dateLabel(row.date)}
                      </TableCell>
                      <TableCell align="right">{money(row.totalRevenue)}</TableCell>
                      <TableCell align="right">{money(row.cashRevenue)}</TableCell>
                      <TableCell align="right">{money(row.onlineRevenue)}</TableCell>
                      <TableCell align="right">{row.transactionCount}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </AccordionDetails>
        </Accordion>
      )}
    </Box>
  );
}

export function PaymentReportChart({ revenue }: { revenue: RevenueByPaymentMethodDto }) {
  if (revenue.total === 0)
    return (
      <Typography color="text.secondary" sx={{ py: 4 }}>
        No payment activity in this period.
      </Typography>
    );
  return (
    <BarChart
      title="Revenue by payment method and sale type in INR"
      height={240}
      layout="horizontal"
      hideLegend
      skipAnimation
      yAxis={[{ scaleType: 'band', data: ['Gaming plans', 'Products'], width: 95 }]}
      xAxis={[{ valueFormatter: compact, tickNumber: 4 }]}
      series={[
        {
          id: 'cash',
          label: 'Cash',
          data: [revenue.planCashRevenue, revenue.productCashRevenue],
          color: '#237a5c',
        },
        {
          id: 'online',
          label: 'Online',
          data: [revenue.planOnlineRevenue, revenue.productOnlineRevenue],
          color: '#598ab1',
        },
        {
          id: 'credit',
          label: 'Running tab',
          data: [revenue.planCreditRevenue, revenue.productCreditRevenue],
          color: '#bd944d',
        },
      ].map((series) => ({
        ...series,
        stack: 'revenue',
        valueFormatter: money,
        highlightScope: { highlight: 'item', fade: 'global' },
      }))}
      grid={{ vertical: true }}
      margin={{ left: 0, right: 10, top: 10, bottom: 0 }}
    />
  );
}

type DeviceMetric = 'utilizationPercentage' | 'totalHours' | 'totalSessions';
const deviceMetrics: {
  id: DeviceMetric;
  label: string;
  format: (value: number | null) => string;
}[] = [
  { id: 'utilizationPercentage', label: 'Utilization', format: (v) => `${(v ?? 0).toFixed(1)}%` },
  { id: 'totalHours', label: 'Hours', format: (v) => `${(v ?? 0).toFixed(1)} h` },
  { id: 'totalSessions', label: 'Sessions', format: (v) => String(v ?? 0) },
];

export function DeviceReportChart({ devices }: { devices: DeviceStatsDto['deviceUtilization'] }) {
  const [metric, setMetric] = useState<DeviceMetric>('utilizationPercentage');
  const sorted = [...devices].sort((a, b) => b[metric] - a[metric]);
  const selected = deviceMetrics.find((item) => item.id === metric) ?? {
    label: 'Utilization',
    format: (v: number | null) => `${(v ?? 0).toFixed(1)}%`,
  };
  return (
    <Box sx={{ minWidth: 0 }}>
      <ToggleButtonGroup
        exclusive
        size="small"
        value={metric}
        aria-label="Device metric"
        onChange={(_, value) => {
          if (value) setMetric(value);
        }}
      >
        {deviceMetrics.map((item) => (
          <ToggleButton key={item.id} value={item.id}>
            {item.label}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>
      {sorted.length ? (
        <BarChart
          title={`Device ${selected.label.toLowerCase()}`}
          layout="horizontal"
          height={Math.max(250, sorted.length * 36 + 70)}
          yAxis={[
            {
              scaleType: 'band',
              data: sorted.map((row) => row.deviceId),
              valueFormatter: (id: string) =>
                sorted.find((row) => row.deviceId === id)?.deviceName ?? id,
              width: 110,
            },
          ]}
          xAxis={[
            {
              min: 0,
              tickNumber: 5,
              ...(metric === 'totalSessions' ? { tickMinStep: 1 } : {}),
              valueFormatter: (v: number) =>
                metric === 'utilizationPercentage' ? `${v}%` : compact(v),
            },
          ]}
          series={[
            {
              label: selected.label,
              data: sorted.map((row) => row[metric]),
              color: '#268268',
              valueFormatter: selected.format,
              highlightScope: { highlight: 'item', fade: 'global' },
            },
          ]}
          hideLegend
          skipAnimation
          grid={{ vertical: true }}
          margin={{ left: 0, right: 15, top: 15, bottom: 0 }}
        />
      ) : (
        <Typography color="text.secondary" sx={{ py: 4 }}>
          No device activity in this period.
        </Typography>
      )}
    </Box>
  );
}
