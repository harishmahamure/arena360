/** Allow digits only (optional leading minus). */
export function filterIntegerInput(raw: string, allowNegative = false): string {
  if (allowNegative) {
    const match = raw.match(/^-?\d*/);
    return match?.[0] ?? '';
  }
  return raw.replace(/\D/g, '');
}

/** Allow digits with one decimal separator and a max fraction length. */
export function filterDecimalInput(raw: string, decimalPlaces = 2, allowNegative = false): string {
  let normalized = raw.replace(/,/g, '.');
  if (allowNegative && normalized.startsWith('-')) {
    normalized = `-${normalized.slice(1).replace(/-/g, '')}`;
  } else {
    normalized = normalized.replace(/-/g, '');
  }

  const parts = normalized.split('.');
  const whole = parts[0] ?? '';
  if (parts.length === 1) {
    return allowNegative && raw.startsWith('-') && whole === '' ? '-' : whole;
  }

  const fraction = (parts[1] ?? '').replace(/\D/g, '').slice(0, decimalPlaces);
  return `${whole}.${fraction}`;
}

/** INR-style currency: non-negative, max 2 decimal places. */
export function filterCurrencyInput(raw: string): string {
  const filtered = filterDecimalInput(raw, 2, false);
  if (filtered.startsWith('-')) {
    return filtered.slice(1);
  }
  return filtered;
}

/** True while the user is mid-entry (e.g. "1." or "-"). */
export function isIncompleteDecimalInput(value: string): boolean {
  if (value === '' || value === '.' || value === '-') return true;
  if (value.endsWith('.')) return true;
  return false;
}

/** Parse a committed decimal field string to number or empty. */
export function parseDecimalFieldValue(value: string): number | '' {
  if (value === '' || value === '.' || value === '-') return '';
  const num = Number(value);
  return Number.isNaN(num) ? '' : num;
}

/** Format a stored number for display in a decimal input. */
export function formatDecimalDisplay(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '';
  return String(value);
}
