import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { normalizeRevenue } from '../../services/stats/statsHelpers';
import { DeviceReportChart, PaymentReportChart, RevenueReportChart } from './DashboardReportCharts';

// Inspect the data handed to the chart engine independently of SVG layout in jsdom.
vi.mock('@mui/x-charts/LineChart', () => ({
  LineChart: (props: unknown) => (
    <output aria-label="Line chart data">{JSON.stringify(props)}</output>
  ),
}));
vi.mock('@mui/x-charts/BarChart', () => ({
  BarChart: (props: unknown) => (
    <output aria-label="Bar chart data">{JSON.stringify(props)}</output>
  ),
}));
afterEach(cleanup);
const rows = [
  {
    date: '2026-08-06',
    totalRevenue: 250.5,
    cashRevenue: 150.5,
    onlineRevenue: 100,
    transactionCount: 3,
  },
  {
    date: '2026-08-04',
    totalRevenue: 100,
    cashRevenue: 40,
    onlineRevenue: 60,
    transactionCount: 2,
  },
];
const chartData = (name: string) => JSON.parse(screen.getByLabelText(name).textContent ?? '{}');

it('sorts reported dates, switches metrics, and keeps at least one revenue series visible', () => {
  render(<RevenueReportChart rows={rows} />);
  expect(chartData('Line chart data').series[0].data).toEqual([100, 250.5]);
  fireEvent.click(screen.getByRole('button', { name: 'Cash' }));
  expect(chartData('Line chart data').series.map((s: { id: string }) => s.id)).toEqual([
    'totalRevenue',
    'cashRevenue',
  ]);
  fireEvent.click(screen.getByRole('button', { name: 'Total revenue' }));
  fireEvent.click(screen.getByRole('button', { name: 'Cash' }));
  expect(chartData('Line chart data').series[0].data).toEqual([40, 150.5]);
  fireEvent.click(screen.getByRole('button', { name: 'Transactions' }));
  expect(chartData('Line chart data').series[0].data).toEqual([2, 3]);
  fireEvent.click(screen.getByRole('button', { name: 'View daily report data' }));
  expect(screen.getByRole('table', { name: 'Daily revenue report' })).toHaveTextContent('₹250.50');
});

it('ranks devices by the chosen metric without clamping or dropping devices', () => {
  render(
    <DeviceReportChart
      devices={[
        {
          deviceId: 'a',
          deviceName: 'PC 1',
          totalHours: 15,
          totalSessions: 3,
          utilizationPercentage: 110,
        },
        {
          deviceId: 'b',
          deviceName: 'PC 2',
          totalHours: 5,
          totalSessions: 8,
          utilizationPercentage: 20,
        },
      ]}
    />,
  );
  expect(chartData('Bar chart data').series[0].data).toEqual([110, 20]);
  fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
  expect(chartData('Bar chart data').yAxis[0].data).toEqual(['b', 'a']);
  expect(chartData('Bar chart data').series[0].data).toEqual([8, 3]);
});

it('preserves payment method and product splits', () => {
  render(
    <PaymentReportChart
      revenue={normalizeRevenue({
        planCashRevenue: 10,
        productCashRevenue: 20,
        planOnlineRevenue: 30,
        productOnlineRevenue: 40,
        planCreditRevenue: 50,
        productCreditRevenue: 60,
      })}
    />,
  );
  expect(chartData('Bar chart data').series.map((s: { data: number[] }) => s.data)).toEqual([
    [10, 20],
    [30, 40],
    [50, 60],
  ]);
});

it('shows actionable empty states without fabricated chart points', () => {
  render(
    <>
      <RevenueReportChart rows={[]} />
      <DeviceReportChart devices={[]} />
      <PaymentReportChart revenue={normalizeRevenue({})} />
    </>,
  );
  expect(screen.getByText(/Try a wider date range/)).toBeInTheDocument();
  expect(screen.getByText(/No device activity/)).toBeInTheDocument();
  expect(screen.getByText(/No payment activity/)).toBeInTheDocument();
  expect(screen.queryByLabelText('Line chart data')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('Bar chart data')).not.toBeInTheDocument();
});
