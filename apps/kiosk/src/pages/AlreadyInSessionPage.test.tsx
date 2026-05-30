import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AlreadyInSessionPage } from './AlreadyInSessionPage';

const dismissConflict = vi.fn();

vi.mock('../context/KioskProvider', () => ({
  useKiosk: () => ({ conflictDevice: 'PC-12', dismissConflict }),
}));

describe('AlreadyInSessionPage', () => {
  it('names the conflicting station', () => {
    render(<AlreadyInSessionPage />);
    expect(screen.getByText(/already in a session/i)).toBeInTheDocument();
    expect(screen.getByText('PC-12')).toBeInTheDocument();
  });

  it('dismisses back to idle', () => {
    render(<AlreadyInSessionPage />);
    fireEvent.click(screen.getByRole('button', { name: /back/i }));
    expect(dismissConflict).toHaveBeenCalled();
  });
});
