import type { BusinessReport } from '../../../services/stats/business';
export const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
export const sum = <T>(rows: T[], value: (row: T) => number) =>
  rows.reduce((total, row) => total + value(row), 0);
export const ratio = (value: number, total: number) => (total > 0 ? value / total : null);
export const percent = (value: number | null) =>
  value === null ? '—' : `${(value * 100).toFixed(1)}%`;
export const money = (value: number | null) =>
  value === null
    ? '—'
    : new Intl.NumberFormat('en-IN', {
        style: 'currency',
        currency: 'INR',
        maximumFractionDigits: 0,
      }).format(value);
export const number = (value: number | null) =>
  value === null ? '—' : new Intl.NumberFormat('en-IN', { maximumFractionDigits: 1 }).format(value);
export function datesBetween(start: string, end: string) {
  const dates: string[] = [];
  for (
    let day = new Date(`${start}T00:00:00Z`);
    day.toISOString().slice(0, 10) <= end;
    day = new Date(day.getTime() + 86400000)
  ) {
    dates.push(day.toISOString().slice(0, 10));
    if (dates.length > 366) throw new Error('Analytics supports at most 366 days');
  }
  return dates;
}
export function weekday(date: string) {
  return (new Date(`${date}T00:00:00Z`).getUTCDay() + 6) % 7;
}
export function summarize(report: BusinessReport) {
  const salesByDay = new Map(report.dailySales.map((row) => [row.date, row]));
  const daily = datesBetween(report.period.startDate, report.period.endDate).map(
    (date) =>
      salesByDay.get(date) ?? {
        date,
        revenue: 0,
        planRevenue: 0,
        posRevenue: 0,
        transactions: 0,
        buyers: 0,
      },
  );
  const previous = report.dailySales.filter((row) => row.date < report.period.startDate);
  const revenue = sum(daily, (row) => row.revenue);
  const priorRevenue = sum(previous, (row) => row.revenue);
  const transactions = sum(daily, (row) => row.transactions);
  const hours = sum(report.hourlyUsage, (row) => row.hours);
  const sessions = sum(report.hourlyUsage, (row) => row.starts);
  return {
    daily,
    revenue,
    priorRevenue,
    transactions,
    hours,
    sessions,
    planRevenue: sum(daily, (row) => row.planRevenue),
    growth: priorRevenue > 0 ? (revenue - priorRevenue) / priorRevenue : null,
    averageTicket: ratio(revenue, transactions),
    elapsedHours: Math.max(
      0,
      (Date.parse(report.period.observedUntil) -
        Date.parse(`${report.period.startDate}T00:00:00+05:30`)) /
        3600000,
    ),
  };
}
export interface CapacityAssumptions {
  openHour: number;
  closeHour: number;
  hourlyRate: number;
  capturePercent: number;
}
export const defaultAssumptions: CapacityAssumptions = {
  openHour: 9,
  closeHour: 24,
  hourlyRate: 100,
  capturePercent: 25,
};
export function capacityModel(report: BusinessReport, assumptions: CapacityAssumptions) {
  const cells = Array.from({ length: 168 }, (_, i) => ({
    weekday: Math.floor(i / 24),
    hour: i % 24,
    used: 0,
    available: 0,
    starts: 0,
  }));
  const until = Date.parse(report.period.observedUntil);
  for (const date of datesBetween(report.period.startDate, report.period.endDate)) {
    for (let h = assumptions.openHour; h < assumptions.closeHour; h++) {
      const from = Date.parse(`${date}T${String(h).padStart(2, '0')}:00:00+05:30`);
      const elapsed = Math.max(0, Math.min(1, (until - from) / 3600000));
      const cell = cells[weekday(date) * 24 + h];
      if (cell) cell.available += elapsed * report.stations.length;
    }
  }
  for (const row of report.hourlyUsage) {
    const cell = cells[(row.weekday - 1) * 24 + row.hour];
    if (cell && row.hour >= assumptions.openHour && row.hour < assumptions.closeHour) {
      cell.used += row.hours;
      cell.starts += row.starts;
    }
  }
  const available = sum(cells, (cell) => cell.available);
  const used = sum(cells, (cell) => cell.used);
  const unused = Math.max(0, available - used);
  return {
    cells,
    available,
    used,
    unused,
    occupancy: ratio(used, available),
    ceiling: unused * assumptions.hourlyRate,
    opportunity: (unused * assumptions.hourlyRate * assumptions.capturePercent) / 100,
    inconsistent: cells.some((cell) => cell.used > cell.available + 0.01),
  };
}
export function pricingScenario(
  used: number,
  capacity: number,
  rate: number,
  priceChange: number,
  demandChange: number,
) {
  const baseline = used * rate;
  const projectedHours = Math.min(capacity, Math.max(0, used * (1 + demandChange / 100)));
  const projected = projectedHours * rate * (1 + priceChange / 100);
  return { baseline, projected, delta: projected - baseline, projectedHours };
}
export function forecast(report: BusinessReport) {
  const { daily } = summarize(report);
  // Partial current days cannot stand in for complete weekday observations.
  const complete = daily.filter(
    (day) =>
      Date.parse(`${day.date}T00:00:00+05:30`) + 86400000 <=
      Date.parse(report.period.observedUntil),
  );
  if (complete.length < 14) return [];
  const end = Date.parse(`${report.period.endDate}T00:00:00Z`);
  return Array.from({ length: 7 }, (_, i) => {
    const date = new Date(end + (i + 1) * 86400000).toISOString().slice(0, 10);
    const samples = complete.filter((day) => weekday(day.date) === weekday(date));
    return {
      date,
      revenue: sum(samples, (day) => day.revenue) / samples.length,
      low: Math.min(...samples.map((day) => day.revenue)),
      high: Math.max(...samples.map((day) => day.revenue)),
      samples: samples.length,
    };
  });
}
