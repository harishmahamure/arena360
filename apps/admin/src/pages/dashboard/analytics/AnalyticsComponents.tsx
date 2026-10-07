import { ExpandMore } from '@mui/icons-material';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Box,
  Card,
  Grid,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import { BarChart } from '@mui/x-charts/BarChart';
import { LineChart } from '@mui/x-charts/LineChart';
import { type ReactNode, useState } from 'react';
import type { BusinessReport } from '../../../services/stats/business';
import {
  type capacityModel,
  datesBetween,
  money,
  number,
  percent,
  ratio,
  summarize,
  weekdays,
} from './analyticsModel';

export function Metrics({ items }: { items: { label: string; value: string; hint?: string }[] }) {
  return (
    <Grid container spacing={2}>
      {items.map((item) => (
        <Grid key={item.label} size={{ xs: 12, sm: 6, xl: 3 }}>
          <Card sx={{ p: 2.5, height: '100%' }}>
            <Typography variant="body2" color="text.secondary">
              {item.label}
            </Typography>
            <Typography variant="h4" sx={{ my: 1 }}>
              {item.value}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {item.hint}
            </Typography>
          </Card>
        </Grid>
      ))}
    </Grid>
  );
}
export function Section({
  title,
  note,
  children,
}: {
  title: string;
  note?: string;
  children: ReactNode;
}) {
  return (
    <Card sx={{ p: { xs: 2, md: 3 }, minWidth: 0 }}>
      <Typography variant="h6" sx={{ mb: 1 }}>
        {title}
      </Typography>
      {note && (
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          {note}
        </Typography>
      )}
      {children}
    </Card>
  );
}
export function DataTable<T extends { id?: string; date?: string; name?: string }>({
  title,
  rows,
  columns,
}: {
  title: string;
  rows: T[];
  columns: { label: string; cell: (row: T) => ReactNode }[];
}) {
  return rows.length ? (
    <TableContainer sx={{ maxHeight: 440 }}>
      <Table size="small" stickyHeader aria-label={title}>
        <TableHead>
          <TableRow>
            {columns.map((col) => (
              <TableCell key={col.label}>{col.label}</TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => (
            <TableRow key={row.id ?? row.date ?? row.name}>
              {columns.map((col, j) => (
                <TableCell
                  key={col.label}
                  component={j === 0 ? 'th' : 'td'}
                  scope={j === 0 ? 'row' : undefined}
                >
                  {col.cell(row)}
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  ) : (
    <Alert severity="info">
      No records for this report in the selected period. Try a wider date range.
    </Alert>
  );
}
export function Bars({
  title,
  rows,
  format = number,
}: {
  title: string;
  rows: { label: string; value: number }[];
  format?: (value: number | null) => string;
}) {
  return rows.length ? (
    <Box sx={{ maxHeight: 500, overflowY: 'auto' }}>
      <BarChart
        title={title}
        layout="horizontal"
        height={Math.max(240, rows.length * 35 + 65)}
        yAxis={[
          {
            scaleType: 'band',
            data: rows.map((_, i) => i),
            valueFormatter: (i: number) => rows[i]?.label ?? '',
            width: 115,
          },
        ]}
        xAxis={[
          {
            tickNumber: 4,
            valueFormatter: (v: number) =>
              new Intl.NumberFormat('en-IN', { notation: 'compact' }).format(v),
          },
        ]}
        series={[
          {
            label: title,
            data: rows.map((row) => row.value),
            valueFormatter: format,
            color: '#268268',
            highlightScope: { highlight: 'item', fade: 'global' },
          },
        ]}
        grid={{ vertical: true }}
        hideLegend
        skipAnimation
        margin={{ left: 0, top: 10, right: 20, bottom: 5 }}
      />
    </Box>
  ) : (
    <Alert severity="info">No activity for the selected period.</Alert>
  );
}
export function RevenueTrend({ report, compare }: { report: BusinessReport; compare: boolean }) {
  const [metric, setMetric] = useState<'revenue' | 'planRevenue' | 'posRevenue'>('revenue');
  const { daily } = summarize(report);
  const previousDates = datesBetween(
    report.period.previousStartDate,
    new Date(Date.parse(`${report.period.startDate}T00:00:00Z`) - 86400000)
      .toISOString()
      .slice(0, 10),
  );
  const sales = new Map(report.dailySales.map((row) => [row.date, row]));
  return (
    <Section
      title="Sales trend"
      note="Completed and credit sales by transaction date. Collections of existing credit are excluded to avoid counting the same sale twice."
    >
      <ToggleButtonGroup
        exclusive
        size="small"
        aria-label="Sales metric"
        value={metric}
        onChange={(_, v) => {
          if (v) setMetric(v);
        }}
      >
        {[
          ['revenue', 'All sales'],
          ['planRevenue', 'Gaming plans'],
          ['posRevenue', 'F&B / POS'],
        ].map(([id, label]) => (
          <ToggleButton key={id} value={id ?? ''}>
            {label}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>
      <LineChart
        title="Daily sales in INR"
        height={290}
        xAxis={[
          {
            scaleType: 'point',
            data: daily.map((r) => r.date),
            valueFormatter: (v: string) => v.slice(5),
            tickLabelInterval: (_, i) => i % Math.max(1, Math.ceil(daily.length / 7)) === 0,
          },
        ]}
        yAxis={[
          {
            width: 65,
            valueFormatter: (v: number) =>
              new Intl.NumberFormat('en-IN', { notation: 'compact' }).format(v),
          },
        ]}
        series={[
          {
            label: 'Selected period',
            data: daily.map((r) => r[metric]),
            color: '#268268',
            valueFormatter: money,
            curve: 'linear' as const,
            showMark: daily.length < 15,
          },
          ...(compare
            ? [
                {
                  label: 'Previous period (aligned days)',
                  data: previousDates.map((date) => sales.get(date)?.[metric] ?? 0),
                  color: '#8a97a2',
                  valueFormatter: money,
                  curve: 'linear' as const,
                  showMark: false,
                },
              ]
            : []),
        ]}
        grid={{ horizontal: true }}
        skipAnimation
      />
      <Accordion disableGutters elevation={0}>
        <AccordionSummary expandIcon={<ExpandMore />}>View daily data</AccordionSummary>
        <AccordionDetails>
          <DataTable
            title="Daily sales"
            rows={daily}
            columns={[
              { label: 'Date (IST)', cell: (r) => r.date },
              { label: 'Sales', cell: (r) => money(r.revenue) },
              { label: 'Plans', cell: (r) => money(r.planRevenue) },
              { label: 'POS', cell: (r) => money(r.posRevenue) },
              { label: 'Transactions', cell: (r) => r.transactions },
            ]}
          />
        </AccordionDetails>
      </Accordion>
    </Section>
  );
}
export function Heatmap({ model }: { model: ReturnType<typeof capacityModel> }) {
  const theme = useTheme();
  const [selected, setSelected] = useState<number | null>(null);
  const cell = selected === null ? null : model.cells[selected];
  return (
    <Stack gap={2}>
      <Typography variant="caption" color="text.secondary">
        Select a slot to inspect occupied hours and estimated capacity. Grey cells are outside the
        assumed operating window or have no observed capacity.
      </Typography>
      <Box sx={{ overflowX: 'auto' }}>
        <Box
          role="group"
          aria-label="Occupancy by weekday and hour"
          sx={{
            display: 'grid',
            gridTemplateColumns: '42px repeat(24, minmax(30px, 1fr))',
            gap: 0.5,
            minWidth: 830,
          }}
        >
          <span />
          {Array.from({ length: 24 }, (_, hour) => hour).map((h) => (
            <Typography key={h} variant="caption" textAlign="center">
              {String(h).padStart(2, '0')}
            </Typography>
          ))}
          {weekdays.map((day, d) => (
            <Box key={day} sx={{ display: 'contents' }}>
              <Typography variant="caption" alignSelf="center">
                {day}
              </Typography>
              {model.cells.slice(d * 24, d * 24 + 24).map((item) => {
                const occupancy = ratio(item.used, item.available);
                const label = `${day} ${String(item.hour).padStart(2, '0')}:00 · ${percent(occupancy)}`;
                return (
                  <Tooltip key={item.hour} title={label}>
                    <Box
                      component="button"
                      type="button"
                      aria-label={label}
                      aria-pressed={selected === d * 24 + item.hour}
                      onClick={() => setSelected(d * 24 + item.hour)}
                      sx={{
                        height: 32,
                        border: 1,
                        borderColor:
                          selected === d * 24 + item.hour ? 'primary.main' : 'transparent',
                        borderRadius: 1,
                        cursor: 'pointer',
                        bgcolor:
                          occupancy === null
                            ? 'action.disabledBackground'
                            : alpha(
                                theme.palette.primary.main,
                                0.08 + 0.82 * Math.min(1, occupancy),
                              ),
                        color:
                          occupancy !== null && occupancy > 0.6
                            ? 'primary.contrastText'
                            : 'text.primary',
                        fontSize: 10,
                        '&:focus-visible': { outline: '2px solid', outlineColor: 'primary.main' },
                      }}
                    >
                      {occupancy === null ? '—' : Math.round(occupancy * 100)}
                    </Box>
                  </Tooltip>
                );
              })}
            </Box>
          ))}
        </Box>
      </Box>
      {cell && (
        <Alert severity="info">
          {weekdays[cell.weekday]} {String(cell.hour).padStart(2, '0')}:00: {number(cell.used)}{' '}
          occupied hours / {number(cell.available)} estimated station-hours · {cell.starts} session
          starts · {percent(ratio(cell.used, cell.available))} occupancy across this period.
        </Alert>
      )}
      {model.inconsistent && (
        <Alert severity="warning">
          Some slots exceed estimated capacity. Check overlapping sessions, station history, and the
          operating-window assumption before using opportunity estimates.
        </Alert>
      )}
    </Stack>
  );
}
export function TrackingGap({ title, items }: { title: string; items: string[] }) {
  return (
    <Section
      title={title}
      note="These metrics are unavailable until the required events are collected. Missing measurements are not shown as zero."
    >
      <Stack spacing={1}>
        {items.map((item) => (
          <Alert severity="info" key={item}>
            {item}
          </Alert>
        ))}
      </Stack>
    </Section>
  );
}
