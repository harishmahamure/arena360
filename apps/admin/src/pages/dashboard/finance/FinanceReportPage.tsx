import { PageHeader, PageShell } from '@gaming-cafe/ui';
import { Download, Refresh } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  LinearProgress,
  Stack,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Link } from 'react-router-dom';
import { financeReportCsv, getFinanceReport, type ReportGroup } from '../../../services/operations';

const date = (d: Date) => d.toISOString().slice(0, 10);
export default function FinanceReportPage() {
  const today = date(new Date());
  const monthStart = `${today.slice(0, 7)}-01`;
  const [range, setRange] = useState({ start: monthStart, end: today });
  const [applied, setApplied] = useState(range);
  const [tab, setTab] = useState('daily');
  const [validation, setValidation] = useState('');
  const report = useQuery({
    queryKey: ['financeReport', applied],
    queryFn: () => getFinanceReport(applied.start, applied.end),
  });
  const data = report.data;
  const dirty = range.start !== applied.start || range.end !== applied.end;
  const money = (value: string) => {
    try {
      return new Intl.NumberFormat(undefined, {
        style: 'currency',
        currency: data?.currency ?? 'INR',
      }).format(Number(value));
    } catch {
      return `${data?.currency ?? ''} ${Number(value).toFixed(2)}`;
    }
  };
  function apply(next = range) {
    const days = (Date.parse(next.end) - Date.parse(next.start)) / 86400000;
    if (!next.start || !next.end || !Number.isFinite(days) || days < 0 || days > 365) {
      setValidation('Choose a start and end date covering at most 366 days.');
      return;
    }
    setValidation('');
    setRange(next);
    setApplied(next);
  }
  function exportCsv() {
    if (!data) return;
    const url = URL.createObjectURL(
      new Blob([financeReportCsv(data)], { type: 'text/csv;charset=utf-8' }),
    );
    const a = document.createElement('a');
    a.href = url;
    a.download = `finance-${data.startDate}-${data.endDate}.csv`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  function groupTable(rows: ReportGroup[], title: string) {
    return (
      <TableContainer>
        <Table aria-label={title}>
          <TableHead>
            <TableRow>
              <TableCell>{title}</TableCell>
              <TableCell align="right">Count</TableCell>
              <TableCell align="right">Amount</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.label}>
                <TableCell sx={{ textTransform: 'capitalize' }}>
                  {row.label.replaceAll('_', ' ')}
                </TableCell>
                <TableCell align="right">{row.count}</TableCell>
                <TableCell align="right">{money(row.amount)}</TableCell>
              </TableRow>
            ))}
            {rows.length === 0 && (
              <TableRow>
                <TableCell colSpan={3}>No activity in this period.</TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </TableContainer>
    );
  }
  return (
    <PageShell>
      <PageHeader
        title="Financial reports"
        description={`Sales, spending, and credit for ${data?.locationLabel ?? 'the selected locations'}. Use the workspace location selector to change the report scope.`}
      />
      <Stack direction="row" flexWrap="wrap" gap={1} sx={{ mb: 3 }}>
        <Button component={Link} to="/finance/reconciliation">
          Reconciliation
        </Button>
        <Button component={Link} to="/finance/deposits">
          Deposits
        </Button>
        <Button component={Link} to="/finance/variance">
          Cash variance
        </Button>
        <Button component={Link} to="/expenses">
          Expenses
        </Button>
        <Box sx={{ flex: 1 }} />
        <Button
          startIcon={<Download />}
          variant="outlined"
          disabled={!data || dirty || report.isFetching || report.isError}
          onClick={exportCsv}
        >
          Export CSV
        </Button>
      </Stack>
      <Card variant="outlined" sx={{ mb: 3 }}>
        <CardContent>
          <Stack direction={{ xs: 'column', sm: 'row' }} gap={2} alignItems={{ sm: 'center' }}>
            <TextField
              label="From (UTC)"
              type="date"
              value={range.start}
              onChange={(e) => setRange({ ...range, start: e.target.value })}
              slotProps={{ inputLabel: { shrink: true } }}
            />
            <TextField
              label="Through (UTC)"
              type="date"
              value={range.end}
              onChange={(e) => setRange({ ...range, end: e.target.value })}
              slotProps={{ inputLabel: { shrink: true } }}
            />
            <Button variant="contained" onClick={() => apply()} disabled={report.isFetching}>
              Apply dates
            </Button>
            <Button
              startIcon={<Refresh />}
              disabled={report.isFetching}
              onClick={() => void report.refetch()}
            >
              Refresh
            </Button>
          </Stack>
          <Stack direction="row" flexWrap="wrap" gap={1} sx={{ mt: 2 }}>
            <Button size="small" onClick={() => apply({ start: today, end: today })}>
              Today
            </Button>
            <Button
              size="small"
              onClick={() =>
                apply({ start: date(new Date(Date.parse(today) - 6 * 86400000)), end: today })
              }
            >
              Last 7 days
            </Button>
            <Button size="small" onClick={() => apply({ start: monthStart, end: today })}>
              This month
            </Button>
            <Chip size="small" label="Inclusive dates · UTC" />
          </Stack>
          {validation && <Alert severity="error">{validation}</Alert>}
          {dirty && (
            <Alert severity="info">Apply your dates to update the report and enable export.</Alert>
          )}
        </CardContent>
      </Card>
      {report.isFetching && <LinearProgress aria-label="Loading report" />}
      {report.error && (
        <Alert
          severity="error"
          action={<Button onClick={() => void report.refetch()}>Retry</Button>}
        >
          {report.error.message}
        </Alert>
      )}
      {data && (
        <>
          <Typography variant="body2" color="text.secondary" sx={{ my: 2 }}>
            Showing {data.startDate} through {data.endDate} · {data.currency} · Updated{' '}
            {new Date(data.generatedAt).toLocaleString()}
          </Typography>
          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: {
                xs: '1fr',
                sm: 'repeat(2,minmax(0,1fr))',
                lg: 'repeat(4,minmax(0,1fr))',
              },
              gap: 2,
              mb: 3,
            }}
          >
            {(
              [
                ['Booked sales', data.sales, `${data.saleCount} completed or credit sales`],
                ['Approved expenses', data.approvedExpenses, 'By original expense date'],
                ['Credit collections', data.creditCollections, 'Received during selected period'],
                [
                  'Current receivables',
                  data.currentOutstanding,
                  'Outstanding now, across all dates',
                ],
              ] as const
            ).map(([label, amount, caption]) => (
              <Card key={label} variant="outlined">
                <CardContent>
                  <Typography variant="body2" color="text.secondary">
                    {label}
                  </Typography>
                  <Typography
                    variant="h5"
                    fontWeight={700}
                    sx={{ my: 1, overflowWrap: 'anywhere' }}
                  >
                    {money(amount)}
                  </Typography>
                  <Typography variant="caption" color="text.secondary">
                    {caption}
                  </Typography>
                </CardContent>
              </Card>
            ))}
          </Box>
          <Alert severity="info" sx={{ mb: 2 }}>
            Sales use their current payment status and original sale date; historical totals may
            change after refunds or payment updates. Credit sales count once, and later settlements
            are shown separately. Amounts use the current venue currency. This is an operational
            report, not a profit or tax statement.
          </Alert>
          <Stack direction={{ xs: 'column', md: 'row' }} gap={2} sx={{ mb: 3 }}>
            <Alert
              severity={data.pendingExpenseCount ? 'warning' : 'success'}
              sx={{ flex: 1 }}
              action={
                <Button component={Link} to="/expenses">
                  Review
                </Button>
              }
            >
              {data.pendingExpenseCount} expenses awaiting approval · {money(data.pendingExpenses)}
            </Alert>
            <Alert severity="info" sx={{ flex: 1 }}>
              Pending sales: {money(data.pendingSales)} · Refunded sale value:{' '}
              {money(data.refundedSales)}
            </Alert>
          </Stack>
          <Card variant="outlined">
            <Tabs
              value={tab}
              onChange={(_, value) => setTab(value)}
              variant="scrollable"
              allowScrollButtonsMobile
            >
              <Tab value="daily" label="Daily activity" />
              <Tab value="sales" label="Sales mix" />
              <Tab value="payments" label="Payment methods" />
              <Tab value="expenses" label="Expense categories" />
            </Tabs>
            {tab === 'daily' && (
              <TableContainer>
                <Table aria-label="Daily financial activity">
                  <TableHead>
                    <TableRow>
                      <TableCell>Date (UTC)</TableCell>
                      <TableCell align="right">Booked sales</TableCell>
                      <TableCell align="right">Approved expenses</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {data.daily.map((row) => (
                      <TableRow key={row.date}>
                        <TableCell sx={{ whiteSpace: 'nowrap' }}>{row.date}</TableCell>
                        <TableCell align="right">{money(row.sales)}</TableCell>
                        <TableCell align="right">{money(row.expenses)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
            {tab === 'sales' && groupTable(data.salesByType, 'Sale type')}
            {tab === 'payments' && (
              <>
                <Alert severity="info" sx={{ m: 2 }}>
                  Sales grouped by the original payment method. These are sale values, not cash
                  receipts; split payments are one group.
                </Alert>
                {groupTable(data.salesByPayment, 'Payment method')}
              </>
            )}
            {tab === 'expenses' && groupTable(data.expensesByCategory, 'Approved expense category')}
          </Card>
        </>
      )}
    </PageShell>
  );
}
