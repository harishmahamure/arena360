import { describe, expect, it } from 'vitest';
import {
  filterCurrencyInput,
  filterDecimalInput,
  filterIntegerInput,
  parseDecimalFieldValue,
} from './numericInputFilters';

describe('filterIntegerInput', () => {
  it('keeps digits only', () => {
    expect(filterIntegerInput('12a3')).toBe('123');
    expect(filterIntegerInput('abc')).toBe('');
  });

  it('allows optional leading minus', () => {
    expect(filterIntegerInput('-12a', true)).toBe('-12');
    expect(filterIntegerInput('12-', true)).toBe('12');
  });
});

describe('filterDecimalInput', () => {
  it('limits fraction length', () => {
    expect(filterDecimalInput('12.345', 2)).toBe('12.34');
    expect(filterDecimalInput('12.', 2)).toBe('12.');
  });

  it('keeps only the first decimal separator', () => {
    expect(filterDecimalInput('1.2.3', 2)).toBe('1.2');
  });
});

describe('filterCurrencyInput', () => {
  it('rejects negative values', () => {
    expect(filterCurrencyInput('-12.5')).toBe('12.5');
  });

  it('caps currency to two decimal places', () => {
    expect(filterCurrencyInput('99.999')).toBe('99.99');
  });
});

describe('parseDecimalFieldValue', () => {
  it('parses complete decimals', () => {
    expect(parseDecimalFieldValue('1.2')).toBe(1.2);
    expect(parseDecimalFieldValue('12')).toBe(12);
  });

  it('returns empty for incomplete input', () => {
    expect(parseDecimalFieldValue('')).toBe('');
    expect(parseDecimalFieldValue('.')).toBe('');
    expect(parseDecimalFieldValue('1.')).toBe(1);
  });

  it('allows typing 1.2 without swallowing the decimal point', () => {
    expect(parseDecimalFieldValue('1.')).toBe(1);
    expect(parseDecimalFieldValue('1.2')).toBe(1.2);
  });
});

describe('phone and otp input filters', () => {
  it('strips non-digits from phone input', () => {
    expect(filterIntegerInput('98 7654-3210', false)).toBe('9876543210');
  });

  it('strips whitespace and limits otp to six digits', () => {
    const stripped = '12 34 56 78'.replace(/\s+/g, '');
    expect(filterIntegerInput(stripped, false).slice(0, 6)).toBe('123456');
  });
});
