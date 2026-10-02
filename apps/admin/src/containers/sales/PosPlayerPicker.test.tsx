import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { getPlayers } from '../../services/players/list';
import { getSessions } from '../../services/sessions/list';
import { PosPlayerPicker } from './PosPlayerPicker';

vi.mock('../../hooks/usePermissions', () => ({
  Permission: { SessionsRead: 'sessions:read' },
  usePermissions: () => ({ can: () => true }),
}));
vi.mock('../../services/sessions/list', () => ({ getSessions: vi.fn() }));
vi.mock('../../services/players/list', () => ({ getPlayers: vi.fn() }));

afterEach(cleanup);
describe('POS player picker', () => {
  it('offers players in session as one-tap choices without searching', async () => {
    vi.mocked(getSessions).mockResolvedValue({
      data: [
        { balance: { player: { id: 'p1', username: 'ravi' } }, device: { name: 'PC-04' } },
        { balance: { player: { id: 'p1', username: 'ravi' } }, device: { name: 'PS-1' } },
      ],
    } as never);
    const onChange = vi.fn();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <PosPlayerPicker value={null} onChange={onChange} />
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByRole('button', { name: 'ravi · PC-04' }));
    expect(onChange).toHaveBeenCalledWith({ id: 'p1', username: 'ravi', station: 'PC-04' });
    expect(screen.getAllByRole('button', { name: /ravi/ })).toHaveLength(1);
    expect(getPlayers).not.toHaveBeenCalled();
  });
});
