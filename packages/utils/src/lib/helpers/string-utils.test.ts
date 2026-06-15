import { describe, expect, it } from 'vitest';
import {
  digitsOnly,
  normalizeUsername,
  sanitizeUsernameInput,
  trimOptional,
  trimValue,
} from './string-utils';

describe('trimValue', () => {
  it('trims leading and trailing whitespace', () => {
    expect(trimValue('  hello  ')).toBe('hello');
    expect(trimValue('   ')).toBe('');
  });
});

describe('sanitizeUsernameInput', () => {
  it('collapses interior spaces without touching edges', () => {
    expect(sanitizeUsernameInput('Pranshu  Jha')).toBe('Pranshu_Jha');
    expect(sanitizeUsernameInput(' Yuvraj ')).toBe(' Yuvraj ');
  });
});

describe('normalizeUsername', () => {
  it('trims and collapses whitespace to underscore', () => {
    expect(normalizeUsername(' Yuvraj ')).toBe('Yuvraj');
    expect(normalizeUsername('Pranshu  Jha')).toBe('Pranshu_Jha');
    expect(normalizeUsername('Sharvil shinde')).toBe('Sharvil_shinde');
  });
});

describe('digitsOnly', () => {
  it('strips non-digits', () => {
    expect(digitsOnly('98 7654 3210')).toBe('9876543210');
  });
});

describe('trimOptional', () => {
  it('returns undefined for blank strings', () => {
    expect(trimOptional('  ')).toBeUndefined();
    expect(trimOptional(' hi ')).toBe('hi');
  });
});
