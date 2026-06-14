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
