import '@testing-library/jest-dom/vitest';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, expect, it, vi } from 'vitest';
import { analyticsDashboards } from '../../../constants/analyticsDashboards';
import { moduleRegistry } from '../../../constants/navItems';
import { WorkspaceShell } from '../../../layouts/WorkspaceShell';
import AnalyticsIndexPage from './AnalyticsIndexPage';
import AnalyticsPage from './AnalyticsPage';
import { fixture } from './analyticsFixture';

const api = vi.hoisted(() => vi.fn());
vi.mock('../../../lib/realtime/RealtimeProvider', () => ({ useRealtimeStatus: () => 'connected' }));
vi.mock('../../../services/config/branding', () => ({ useBranding: () => ({ name: 'Arena360' }) }));
vi.mock('../../../components/BrandMark', () => ({ BrandMark: () => null }));
vi.mock('../../../theme/AppearanceDialog', () => ({ default: () => null }));
vi.mock('../../../services/stats/business', () => ({ getBusinessReport: api }));
vi.mock('@mui/x-charts/LineChart', () => ({ LineChart: () => <div>Line chart</div> }));
vi.mock('@mui/x-charts/BarChart', () => ({ BarChart: () => <div>Bar chart</div> }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
function mount(id = 'executive', report = fixture) {
  api.mockResolvedValue(report);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <LocalizationProvider dateAdapter={AdapterDateFns}>
        <MemoryRouter initialEntries={[`/analytics/${id}?startDate=2026-09-01&endDate=2026-09-14`]}>
          <WorkspaceShell
            navItems={moduleRegistry.filter((item) => item.path === '/analytics')}
            pageTitle="Business dashboard"
            user={{ name: 'Demo', email: '', role: 'Admin' }}
            onLogout={() => {}}
            appBarQuickActions={{
              showPos: false,
              showPlan: false,
              onPosClick: () => {},
              onPlanClick: () => {},
            }}
          >
            <Routes>
              <Route path="/analytics">
                <Route index element={<AnalyticsIndexPage />} />
                {analyticsDashboards.map((d) => (
                  <Route key={d.id} path={d.id} element={<AnalyticsPage dashboard={d.id} />} />
                ))}
              </Route>
              <Route path="*" element={<h1>Page not found</h1>} />
            </Routes>
          </WorkspaceShell>
        </MemoryRouter>
      </LocalizationProvider>
    </QueryClientProvider>,
  );
}
it('exposes all 11 dashboards and shares applied dates while navigating', async () => {
  mount();
  await screen.findByText('Sales revenue');
  expect(screen.getByRole('heading', { name: 'Executive Overview', level: 1 })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Collapse Business dashboard' })).toHaveAttribute(
    'aria-expanded',
    'true',
  );
  expect(api).toHaveBeenCalledWith('2026-09-01', '2026-09-14');
  for (const d of analyticsDashboards)
    expect(screen.getByRole('link', { name: d.title })).toHaveAttribute(
      'href',
      `/analytics/${d.id}?startDate=2026-09-01&endDate=2026-09-14`,
    );
  fireEvent.click(screen.getByRole('link', { name: 'Customer Retention' }));
  await screen.findByText('New visitors');
  expect(screen.getByRole('link', { name: 'Customer Retention' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  expect(screen.getByRole('heading', { name: 'Customer Retention', level: 1 })).toBeInTheDocument();
  expect(api).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole('link', { name: 'Game Analytics' })).not.toBeInTheDocument();
  expect(screen.queryByRole('link', { name: 'Promotions' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Search or jump to a page' }));
  const search = screen.getByRole('textbox', { name: 'Search pages and actions' });
  fireEvent.change(search, { target: { value: 'Executive Overview' } });
  expect(fireEvent.keyDown(search, { key: 'Enter' })).toBe(false);
  await screen.findByText('Sales revenue');
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(api).toHaveBeenCalledTimes(1);
});
it('edits scenario inputs without fetching or changing live pricing', async () => {
  mount('pricing');
  await screen.findByText('Price and demand scenario');
  const slider = screen.getByRole('slider', { name: 'Price change percent' });
  fireEvent.change(slider, { target: { value: 25 } });
  expect(screen.getByText('Price change: 25%')).toBeInTheDocument();
  expect(api).toHaveBeenCalledTimes(1);
  fireEvent.change(screen.getByLabelText('Potential rate / hour (INR)'), {
    target: { value: 200 },
  });
  expect(api).toHaveBeenCalledTimes(1);
});
it('keeps draft date presets out of API calls until Apply', async () => {
  mount();
  await screen.findByText('Sales revenue');
  fireEvent.click(screen.getByRole('button', { name: '7 days' }));
  expect(api).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
  await waitFor(() => expect(api).toHaveBeenCalledTimes(2));
});
it('withholds financial scenarios when occupied hours exceed the capacity model', async () => {
  mount('opportunity', {
    ...fixture,
    hourlyUsage: [{ date: '2026-09-01', weekday: 2, hour: 10, hours: 1000, starts: 5 }],
  });
  await screen.findByText(/Recorded usage exceeds the estimated inventory capacity/);
  expect(screen.queryByText('Scenario gross opportunity')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('link', { name: 'Dynamic Pricing' }));
  await screen.findByText(/Scenarios are unavailable for this range/);
  expect(screen.queryByRole('slider', { name: 'Price change percent' })).not.toBeInTheDocument();
});

it('opens a report directory at the parent URL and preserves dates through its cards', async () => {
  mount('');
  const directory = screen.getByRole('navigation', { name: 'Business reports' });
  expect(within(directory).getAllByRole('link')).toHaveLength(11);
  expect(api).not.toHaveBeenCalled();
  fireEvent.click(within(directory).getByRole('link', { name: 'Executive Overview' }));
  await screen.findByText('Sales revenue');
  expect(api).toHaveBeenCalledWith('2026-09-01', '2026-09-14');
  const breadcrumb = screen
    .getAllByRole('link', { name: 'Business dashboard' })
    .find((link) => link.closest('.MuiBreadcrumbs-root'));
  expect(breadcrumb).toHaveAttribute('href', '/analytics?startDate=2026-09-01&endDate=2026-09-14');
  if (!breadcrumb) throw new Error('Business dashboard breadcrumb missing');
  fireEvent.click(breadcrumb);
  expect(screen.getByRole('navigation', { name: 'Business reports' })).toBeInTheDocument();
});
it.each([
  'missing',
  'games',
  'promotions',
])('shows not found for the unavailable subpage %s', (id) => {
  mount(id);
  expect(screen.getByRole('heading', { name: 'Page not found' })).toBeInTheDocument();
  expect(api).not.toHaveBeenCalled();
});
