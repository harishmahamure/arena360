import { http } from '@gaming-cafe/utils';
export type KitchenStatus = 'queued' | 'preparing' | 'ready' | 'served' | 'cancelled';
export interface KitchenTicket {
  id: string;
  transactionId: string;
  status: KitchenStatus;
  revision: number;
  customer: string;
  notes: string | null;
  paymentStatus: string;
  createdAt: string;
  updatedAt: string;
  dueAt: string;
  items: {
    productId: string;
    name: string;
    quantity: number;
    station: string;
    options?: string[];
  }[];
  events: { status: KitchenStatus; actor: string | null; reason: string | null; at: string }[];
}
export interface KitchenMenuItem {
  productId: string;
  name: string;
  enabled: boolean;
  station: string;
  prepMinutes: number;
  revision: number;
}
const kitchen = '/kiosk-orders/kitchen';
export const getKitchenTickets = (history: boolean) =>
  http.get<KitchenTicket[]>(`${kitchen}/tickets`, { params: { history } });
export const advanceKitchenTicket = (
  ticket: KitchenTicket,
  status: KitchenStatus,
  reason?: string,
) =>
  http.patch(`${kitchen}/tickets/${ticket.id}`, {
    status,
    expectedRevision: ticket.revision,
    reason,
  });
export const getKitchenMenu = () => http.get<KitchenMenuItem[]>(`${kitchen}/menu`);
export const saveKitchenMenu = (item: KitchenMenuItem) =>
  http.put(`${kitchen}/menu/${item.productId}`, {
    enabled: item.enabled,
    station: item.station,
    prepMinutes: item.prepMinutes,
    expectedRevision: item.revision,
  });
export interface ReportGroup {
  label: string;
  amount: string;
  count: number;
}
export interface FinanceReport {
  locationLabel?: string;
  generatedAt: string;
  startDate: string;
  endDate: string;
  timezone: string;
  currency: string;
  sales: string;
  saleCount: number;
  approvedExpenses: string;
  pendingExpenses: string;
  pendingExpenseCount: number;
  refundedSales: string;
  pendingSales: string;
  currentOutstanding: string;
  creditCollections: string;
  salesByType: ReportGroup[];
  salesByPayment: ReportGroup[];
  expensesByCategory: ReportGroup[];
  daily: { date: string; sales: string; expenses: string }[];
}
export const getFinanceReport = (startDate: string, endDate: string) =>
  http.get<FinanceReport>('/stats/finance/report', { params: { startDate, endDate } });

// Prefix spreadsheet formulas in user-authored labels and quote every field.
export const csvCell = (value: string | number) => {
  const text = String(value);
  return `"${(/^[\s]*[=+\-@]/.test(text) ? `'${text}` : text).replaceAll('"', '""')}"`;
};
export function financeReportCsv(report: FinanceReport) {
  const rows: (string | number)[][] = [
    ['Finance report', report.startDate, report.endDate, report.timezone, report.currency],
    ['Locations', report.locationLabel ?? 'All accessible locations'],
    ['Generated at', report.generatedAt],
    [
      'Basis',
      'Current payment status; sales and expenses attributed to their original dates. Not an accounting profit statement.',
    ],
    ['Metric', 'Amount'],
    ['Booked sales', report.sales],
    ['Approved expenses', report.approvedExpenses],
    ['Pending expenses', report.pendingExpenses],
    ['Refunded sale value', report.refundedSales],
    ['Pending sale value', report.pendingSales],
    ['Current outstanding (at generation)', report.currentOutstanding],
    ['Credit collections (settlement date)', report.creditCollections],
    [],
    ['Date (UTC)', 'Sales', 'Approved expenses'],
    ...report.daily.map((row) => [row.date, row.sales, row.expenses]),
  ];
  for (const [label, groups] of [
    ['Sales by type', report.salesByType],
    ['Sales by payment method (not receipts)', report.salesByPayment],
    ['Approved expenses by category', report.expensesByCategory],
  ] as const) {
    rows.push(
      [],
      [label, 'Amount', 'Count'],
      ...groups.map((row) => [row.label, row.amount, row.count]),
    );
  }
  return '\uFEFF' + rows.map((row) => row.map(csvCell).join(',')).join('\r\n');
}
