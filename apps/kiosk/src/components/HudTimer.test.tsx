import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { HudTimer } from './HudTimer';

describe('HudTimer', () => {
  it('formats whole and fractional minutes as m:ss', () => {
    render(<HudTimer remainingMinutes={5.5} />);
    expect(screen.getByText('5:30')).toBeInTheDocument();
  });

  it('formats hours when over 60 minutes', () => {
    render(<HudTimer remainingMinutes={75} />);
    expect(screen.getByText('1:15:00')).toBeInTheDocument();
  });

  it('applies the critical tone in the final minute', () => {
    const { container } = render(<HudTimer remainingMinutes={0.5} />);
    expect(container.querySelector('.hud-timer-critical')).not.toBeNull();
  });

  it('applies the warning tone under five minutes', () => {
    const { container } = render(<HudTimer remainingMinutes={4} />);
    expect(container.querySelector('.hud-timer-warning')).not.toBeNull();
  });
});
