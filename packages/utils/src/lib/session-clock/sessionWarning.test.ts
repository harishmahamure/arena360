import { describe, expect, it } from 'vitest';
import { getSessionUrgentThreshold, shouldEmitSessionWarning } from './sessionWarning.js';

describe('getSessionUrgentThreshold', () => {
  it('returns null when above all bands', () => {
    expect(getSessionUrgentThreshold(10.1)).toBeNull();
    expect(getSessionUrgentThreshold(30)).toBeNull();
  });

  it('returns the most urgent band for the current remaining time', () => {
    expect(getSessionUrgentThreshold(10)).toBe(10);
    expect(getSessionUrgentThreshold(8)).toBe(10);
    expect(getSessionUrgentThreshold(5)).toBe(5);
    expect(getSessionUrgentThreshold(3)).toBe(5);
    expect(getSessionUrgentThreshold(1)).toBe(1);
    expect(getSessionUrgentThreshold(0.9)).toBe(1);
  });

  it('returns null at or below zero', () => {
    expect(getSessionUrgentThreshold(0)).toBeNull();
    expect(getSessionUrgentThreshold(-1)).toBeNull();
  });
});

describe('shouldEmitSessionWarning', () => {
  it('emits on first observation for an urgent band (page load)', () => {
    expect(shouldEmitSessionWarning(undefined, 1)).toBe(true);
    expect(shouldEmitSessionWarning(undefined, 10)).toBe(true);
  });

  it('does not emit when remaining in the same band', () => {
    expect(shouldEmitSessionWarning(10, 10)).toBe(false);
    expect(shouldEmitSessionWarning(5, 5)).toBe(false);
    expect(shouldEmitSessionWarning(1, 1)).toBe(false);
  });

  it('emits when crossing into a more urgent band', () => {
    expect(shouldEmitSessionWarning(10, 5)).toBe(true);
    expect(shouldEmitSessionWarning(5, 1)).toBe(true);
  });

  it('does not emit when above all bands', () => {
    expect(shouldEmitSessionWarning(10, null)).toBe(false);
    expect(shouldEmitSessionWarning(null, null)).toBe(false);
  });
});
