import type { SettingDefinition } from '../../../services/config';

export function settingLabel(key: string): string {
  return (key.split('.').at(-1) ?? key)
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}
export function displayValue(value: unknown): string {
  if (value == null) return '';
  return typeof value === 'string' ? value : JSON.stringify(value);
}
export function parseSettingValue(definition: SettingDefinition, raw: string): unknown {
  const value = raw.trim();
  const label = settingLabel(definition.key);
  if (definition.valueType === 'number' || definition.valueType === 'integer') {
    const number = Number(value);
    if (!value || !Number.isFinite(number)) throw new Error(`${label} must be a number.`);
    if (definition.valueType === 'integer' && !Number.isSafeInteger(number))
      throw new Error(`${label} must be a whole number.`);
    const { minimum, maximum } = definition.validation;
    if (typeof minimum === 'number' && number < minimum)
      throw new Error(`Enter ${minimum} or more.`);
    if (typeof maximum === 'number' && number > maximum)
      throw new Error(`Enter ${maximum} or less.`);
    return number;
  }
  if (definition.valueType === 'boolean') {
    if (value !== 'true' && value !== 'false') throw new Error('Choose enabled or disabled.');
    return value === 'true';
  }
  if (definition.valueType === 'uuid') {
    if (!value) return null;
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value))
      throw new Error('Enter a valid identifier.');
  }
  if (definition.valueType === 'timezone') {
    try {
      new Intl.DateTimeFormat('en', { timeZone: value });
    } catch {
      throw new Error('Enter a valid timezone, such as Asia/Kolkata.');
    }
    if (!value) throw new Error('Enter a timezone.');
  }
  if (definition.valueType === 'currency' && !/^[A-Z]{3}$/.test(value))
    throw new Error('Use a three-letter currency code, such as INR.');
  if (definition.validation.format === 'HH:MM' && !/^([01]\d|2[0-3]):[0-5]\d$/.test(value))
    throw new Error('Use 24-hour time, such as 23:00.');
  if (
    typeof definition.validation.maxLength === 'number' &&
    value.length > definition.validation.maxLength
  )
    throw new Error(`Use ${definition.validation.maxLength} characters or fewer.`);
  return value;
}
