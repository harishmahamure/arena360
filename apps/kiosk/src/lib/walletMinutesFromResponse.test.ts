import { describe, expect, it } from 'vitest';
import { walletMinutesFromResponse } from './walletMinutesFromResponse';

describe('walletMinutesFromResponse', () => {
  it('returns raw wallet minutes when provided', () => {
    expect(walletMinutesFromResponse(120)).toBe(120);
  });

  it('falls back to remainingMinutes when wallet minutes are missing', () => {
    expect(walletMinutesFromResponse(undefined, 45)).toBe(45);
  });

  it('returns 0 when both values are missing', () => {
    expect(walletMinutesFromResponse(undefined)).toBe(0);
  });

  it('prefers wallet minutes over remainingMinutes', () => {
    expect(walletMinutesFromResponse(100, 45)).toBe(100);
  });
});
