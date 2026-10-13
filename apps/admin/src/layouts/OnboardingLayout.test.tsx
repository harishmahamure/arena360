import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, expect, it, vi } from 'vitest';
import OnboardingLayout from './OnboardingLayout';

const auth = vi.hoisted(() => ({ token: 'signed-in', role: 'admin' }));
vi.mock('../hooks/store', () => ({
  useSelector: (selector: (state: unknown) => unknown) => selector({ auth: { role: auth.role } }),
}));
vi.mock('@gaming-cafe/utils', () => ({ local: { get: () => auth.token } }));
vi.mock('../components/BrandMark', () => ({ BrandMark: () => <span>Arena360</span> }));
afterEach(cleanup);

function setup() {
  render(
    <MemoryRouter initialEntries={['/onboarding']}>
      <Routes>
        <Route element={<OnboardingLayout />}>
          <Route path="/onboarding" element={<div>Location form</div>} />
        </Route>
        <Route path="/login" element={<div>Sign in</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

it('renders authenticated setup in its own main area without workspace navigation', () => {
  auth.token = 'signed-in';
  auth.role = 'admin';
  setup();
  expect(screen.getByRole('main')).toHaveTextContent('Location form');
  expect(screen.getByText('Arena360')).toBeInTheDocument();
  expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'New sale' })).not.toBeInTheDocument();
});

it('requires a signed-in session for the standalone route', () => {
  auth.token = '';
  setup();
  expect(screen.getByText('Sign in')).toBeInTheDocument();
  expect(screen.queryByText('Location form')).not.toBeInTheDocument();
});
