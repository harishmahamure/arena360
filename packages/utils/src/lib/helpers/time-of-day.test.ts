import { describe, expect, it } from 'vitest';
import {
  formatTimeOfDay,
  normalizeTimeOfDay,
  parseTimeOfDay,
  toTimeInputValue,
} from './time-of-day';

describe('time-of-day helpers', () => {
  it('normalizeTimeOfDay pads HH:MM to HH:MM:SS', () => {
    expect(normalizeTimeOfDay('18:00')).toBe('18:00:00');
    expect(normalizeTimeOfDay('18:00:00')).toBe('18:00:00');
    expect(normalizeTimeOfDay(undefined)).toBe('');
  });

  it('toTimeInputValue strips seconds', () => {
    expect(toTimeInputValue('18:00:00')).toBe('18:00');
    expect(toTimeInputValue('7:30')).toBe('07:30');
  });

  it('parseTimeOfDay and formatTimeOfDay round-trip', () => {
    const parsed = parseTimeOfDay('18:30:45');
    expect(parsed).not.toBeNull();
    expect(formatTimeOfDay(parsed)).toBe('18:30:45');
  });

  it('parseTimeOfDay accepts HH:MM', () => {
    const parsed = parseTimeOfDay('07:15');
    expect(parsed).not.toBeNull();
    expect(formatTimeOfDay(parsed)).toBe('07:15:00');
  });

  it('parseTimeOfDay returns null for invalid input', () => {
    expect(parseTimeOfDay('invalid')).toBeNull();
    expect(parseTimeOfDay('')).toBeNull();
  });
});
