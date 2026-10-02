import '@testing-library/jest-dom/vitest';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter, useLocation, useNavigate } from 'react-router-dom';
import { afterEach, expect, it, vi } from 'vitest';
import { useStatsDateRange } from '../../hooks/useStatsDateRange';
import { StatsDateRangeToolbar } from './StatsDateRangeToolbar';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
function Report() {
  const range = useStatsDateRange();
  return (
    <>
      <StatsDateRangeToolbar
        {...range}
        onRangeChange={range.setRange}
        onCompareChange={range.setCompare}
        onPreset={range.applyPreset}
        onApply={range.apply}
      />
      <output aria-label="Applied filters">{JSON.stringify(range.apiFilters)}</output>
    </>
  );
}
const initial = '/?startDate=2026-08-04&endDate=2026-08-10&venue=demo';
function Wrapper({ children }: { children: ReactNode }) {
  return (
    <MemoryRouter initialEntries={[initial]}>
      <LocalizationProvider dateAdapter={AdapterDateFns}>{children}</LocalizationProvider>
    </MemoryRouter>
  );
}

it.each([
  true,
  false,
])('advances start to end and waits for Apply (desktop=%s)', async (desktop) => {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: desktop && query.includes('pointer: fine'),
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
  render(<Report />, { wrapper: Wrapper });
  const original = screen.getByLabelText('Applied filters').textContent;
  const applyButton = screen.getByRole('button', { name: 'Apply' });
  fireEvent.click(screen.getByRole('button', { name: 'Choose start date' }));
  fireEvent.click(screen.getByRole('gridcell', { name: '15' }));
  await waitFor(() => expect(screen.getByRole('dialog')).toHaveAccessibleName('To'));
  expect(applyButton).toBeDisabled();
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent(original ?? '');
  // End dates before the newly selected start are unavailable.
  expect(screen.getByRole('gridcell', { name: '14' })).toBeDisabled();
  fireEvent.click(screen.getByRole('gridcell', { name: '20' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent(original ?? '');
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent('2026-08-15T00:00:00+05:30');
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent('2026-08-20T23:59:59+05:30');
}, 20000);

it('stages presets and comparison without changing applied filters', () => {
  render(<Report />, { wrapper: Wrapper });
  const original = screen.getByLabelText('Applied filters').textContent;
  fireEvent.click(screen.getByRole('button', { name: '30 days' }));
  fireEvent.click(screen.getByRole('switch', { name: 'Compare previous period' }));
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent(original ?? '');
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
  expect(screen.getByLabelText('Applied filters')).toHaveTextContent('"compare":false');
  expect(screen.getByRole('button', { name: 'Apply' })).toBeDisabled();
});

it('rejects invalid ranges, preserves other URL filters, and syncs navigation', () => {
  const { result } = renderHook(
    () => ({ range: useStatsDateRange(), location: useLocation(), navigate: useNavigate() }),
    { wrapper: Wrapper },
  );
  const original = result.current.range.apiFilters;
  act(() => result.current.range.setRange('2026-02-30', '2026-08-10'));
  act(() => result.current.range.apply());
  expect(result.current.range.apiFilters).toEqual(original);
  act(() => result.current.range.setRange('2026-08-20', '2026-08-10'));
  act(() => result.current.range.apply());
  expect(result.current.range.apiFilters).toEqual(original);
  act(() => result.current.range.setRange('2026-08-01', '2026-08-01'));
  act(() => result.current.range.apply());
  expect(result.current.location.search).toContain('venue=demo');
  act(() => result.current.navigate('/?startDate=2026-09-01&endDate=2026-09-30&compare=false'));
  expect(result.current.range.startDate).toBe('2026-09-01');
  expect(result.current.range.endDate).toBe('2026-09-30');
  expect(result.current.range.compare).toBe(false);
  act(() => result.current.navigate('/?startDate=garbage&endDate=2026-01-01'));
  expect(result.current.range.apiFilters.startDate).not.toContain('garbage');
});
