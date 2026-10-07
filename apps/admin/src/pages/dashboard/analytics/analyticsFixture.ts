import type { BusinessReport } from '../../../services/stats/business';
export const fixture: BusinessReport = {
  period: {
    startDate: '2026-09-01',
    endDate: '2026-09-14',
    previousStartDate: '2026-08-18',
    observedUntil: '2026-09-14T18:30:00Z',
    timezone: 'Asia/Kolkata',
  },
  generatedAt: '2026-09-15T00:00:00Z',
  dailySales: [
    {
      date: '2026-08-20',
      revenue: 50,
      planRevenue: 40,
      posRevenue: 10,
      transactions: 1,
      buyers: 1,
    },
    {
      date: '2026-09-01',
      revenue: 100,
      planRevenue: 75,
      posRevenue: 25,
      transactions: 2,
      buyers: 1,
    },
  ],
  hourlyUsage: [{ date: '2026-09-01', weekday: 2, hour: 10, hours: 1, starts: 1 }],
  stations: [
    { id: 'pc1', name: 'PC 1', location: 'Floor', status: 'available', sessions: 1, hours: 1 },
  ],
  customers: {
    visitors: 1,
    newVisitors: 1,
    repeatVisitors: 0,
    frequentVisitors: 0,
    atRisk: 0,
    priorVisitors: 0,
    retainedVisitors: 0,
  },
  customerDetails: [],
  plans: [],
  wallets: { holders: 0, activeWallets: 0, expiring7Days: 0, expiredWallets: 0, remainingHours: 0 },
  products: [],
  pos: { revenue: 25, transactions: 1, visitingCustomers: 1, attachedCustomers: 1 },
  staff: [],
};
