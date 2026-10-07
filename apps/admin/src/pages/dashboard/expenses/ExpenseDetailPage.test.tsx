import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, expect, it, vi } from 'vitest';
import ExpenseDetailPage from './ExpenseDetailPage';

vi.mock('../../../services/expenses', () => ({
  getExpense: vi.fn().mockResolvedValue({
    id: 'expense-1',
    amount: 100,
    paymentMethod: 'cash',
    description: 'Supplies',
    approvalStatus: 'pending',
    expenseDate: '2026-10-02T00:00:00Z',
    createdAt: '2026-10-02T00:00:00Z',
  }),
  updateExpense: vi.fn(),
  approveExpense: vi.fn(),
  rejectExpense: vi.fn(),
}));
afterEach(cleanup);
it('keeps a cleared amount empty and requires valid, saved edits before approval', async () => {
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <MemoryRouter initialEntries={['/expenses/expense-1']}>
        <Routes>
          <Route path="/expenses/:id" element={<ExpenseDetailPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  const amount = await screen.findByLabelText('Amount');
  await waitFor(() => expect(amount).toHaveValue('100'));
  fireEvent.change(amount, { target: { value: '' } });
  expect(amount).toHaveValue('');
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  expect(screen.getByText('Enter a positive expense amount.')).toBeInTheDocument();
  fireEvent.change(amount, { target: { value: '200' } });
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
});
