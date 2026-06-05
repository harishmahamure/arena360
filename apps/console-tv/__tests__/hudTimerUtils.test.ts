import { formatClock, toneForMinutes } from '../src/components/hudTimerUtils';

describe('formatClock', () => {
  it('formats minutes and seconds', () => {
    expect(formatClock(125)).toBe('2:05');
  });

  it('formats hours when needed', () => {
    expect(formatClock(3661)).toBe('1:01:01');
  });

  it('never shows negative values', () => {
    expect(formatClock(-10)).toBe('0:00');
  });
});

describe('toneForMinutes', () => {
  it('returns critical under one minute', () => {
    expect(toneForMinutes(0.5)).toBe('critical');
  });

  it('returns warning under five minutes', () => {
    expect(toneForMinutes(3)).toBe('warning');
  });

  it('returns normal otherwise', () => {
    expect(toneForMinutes(30)).toBe('normal');
  });
});
