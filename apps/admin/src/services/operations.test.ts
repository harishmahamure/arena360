import { describe, expect, it } from 'vitest';
import { csvCell, type FinanceReport, financeReportCsv } from './operations';

describe('report exports', () => {
  it('escapes spreadsheet formulas, commas, quotes and line breaks', () => {
    expect(csvCell('=SUM(A1)')).toBe('"\'=SUM(A1)"');
    expect(csvCell('  @bad')).toBe('"\'  @bad"');
    expect(csvCell('A,"B"\nC')).toBe('"A,""B""\nC"');
    expect(csvCell('123.4500')).toBe('"123.4500"');
  });
  it('exports server decimals, the applied period and accounting basis', () => {
    const report: FinanceReport = {
      startDate: '2026-10-01',
      endDate: '2026-10-02',
      timezone: 'UTC',
      currency: 'INR',
      generatedAt: '2026-10-02T12:00:00Z',
      sales: '0.30',
      saleCount: 1,
      approvedExpenses: '0',
      pendingExpenses: '0',
      pendingExpenseCount: 0,
      refundedSales: '0',
      pendingSales: '0',
      currentOutstanding: '0',
      creditCollections: '0',
      daily: [{ date: '2026-10-01', sales: '0.30', expenses: '0' }],
      salesByType: [],
      salesByPayment: [],
      expensesByCategory: [{ label: '=Danger', amount: '0.10', count: 1 }],
    };
    const csv = financeReportCsv(report);
    expect(csv).toContain('"2026-10-01","2026-10-02","UTC","INR"');
    expect(csv).toContain('"Booked sales","0.30"');
    expect(csv).toContain('"\'=Danger","0.10","1"');
    expect(csv).toContain('Not an accounting profit statement.');
  });
});
