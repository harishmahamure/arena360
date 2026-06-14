import { describe, expect, it } from 'vitest';
import { filterCurrencyInput, filterDecimalInput, filterIntegerInput } from './numericInputFilters';

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
