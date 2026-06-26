import { describe, expect, it } from 'vitest';
import { walletMinutesFromResponse } from './walletMinutesFromResponse';

describe('walletMinutesFromResponse', () => {
  it('returns raw wallet minutes when provided', () => {
    expect(walletMinutesFromResponse(120)).toBe(120);
  });

  it('returns 0 when wallet minutes are missing', () => {
    expect(walletMinutesFromResponse(undefined)).toBe(0);
  });

  it('does not accept effective remaining minutes as wallet', () => {
    // Effective display (45) must not be passed as wallet — caller supplies wallet only.
    expect(walletMinutesFromResponse(100)).not.toBe(45);
  });
});
