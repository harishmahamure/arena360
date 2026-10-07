import { http } from '@gaming-cafe/utils';
export interface BusinessReport {
  period: {
    startDate: string;
    endDate: string;
    previousStartDate: string;
    observedUntil: string;
    timezone: string;
  };
  generatedAt: string;
  dailySales: {
    date: string;
    revenue: number;
    planRevenue: number;
    posRevenue: number;
    transactions: number;
    buyers: number;
  }[];
  hourlyUsage: { date: string; weekday: number; hour: number; hours: number; starts: number }[];
  stations: {
    id: string;
    name: string;
    location: string;
    status: string;
    sessions: number;
    hours: number;
  }[];
  customers: {
    visitors: number;
    newVisitors: number;
    repeatVisitors: number;
    frequentVisitors: number;
    atRisk: number;
    priorVisitors: number;
    retainedVisitors: number;
  };
  customerDetails: {
    id: string;
    name: string;
    visits: number;
    firstVisit: string;
    lastVisit: string;
    spend: number;
    daysAbsent: number;
  }[];
  plans: {
    id: string;
    name: string;
    purchases: number;
    buyers: number;
    repeatBuyers: number;
    revenue: number;
    hoursSold: number;
  }[];
  wallets: {
    holders: number;
    activeWallets: number;
    expiring7Days: number;
    expiredWallets: number;
    remainingHours: number;
  };
  products: { id: string; name: string; quantity: number; revenue: number; transactions: number }[];
  pos: {
    revenue: number;
    transactions: number;
    visitingCustomers: number;
    attachedCustomers: number;
  };
  staff: {
    id: string;
    name: string;
    shiftHours: number;
    revenue: number;
    transactions: number;
    sessions: number;
  }[];
}
export const getBusinessReport = (startDate: string, endDate: string) =>
  http.get<BusinessReport>('/stats/business', { params: { startDate, endDate } });
