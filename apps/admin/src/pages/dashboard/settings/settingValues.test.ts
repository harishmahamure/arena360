import { describe, expect, it } from 'vitest';
import type { SettingDefinition } from '../../../services/config';
import { parseSettingValue } from './settingValues';

const definition = (
  valueType: SettingDefinition['valueType'],
  validation = {},
): SettingDefinition => ({
  key: 'sessions.warning',
  category: 'sessions',
  description: '',
  valueType,
  defaultValue: 0,
  allowedScopes: ['organization', 'location'],
  validation,
  sensitive: false,
  owner: 'venue',
});
describe('typed setting values', () => {
  it.each([
    '',
    ' ',
    '12abc',
    'Infinity',
  ])('rejects invalid numeric input %j without truncating it', (raw) => {
    expect(() => parseSettingValue(definition('number'), raw)).toThrow();
  });
  it('rejects fractional integer input rather than rounding it', () => {
    expect(() => parseSettingValue(definition('integer'), '3.5')).toThrow('whole number');
  });
  it('enforces catalog bounds', () => {
    expect(() =>
      parseSettingValue(definition('number', { minimum: 0, maximum: 100 }), '101'),
    ).toThrow('100 or less');
    expect(parseSettingValue(definition('number', { minimum: 0 }), '0')).toBe(0);
  });
  it('requires a valid boolean instead of silently disabling', () => {
    expect(() => parseSettingValue(definition('boolean'), 'yes')).toThrow();
    expect(parseSettingValue(definition('boolean'), 'false')).toBe(false);
  });
  it('validates time and timezone inputs', () => {
    expect(() => parseSettingValue(definition('string', { format: 'HH:MM' }), '24:60')).toThrow();
    expect(() => parseSettingValue(definition('timezone'), 'not-a-zone')).toThrow();
    expect(parseSettingValue(definition('timezone'), 'Asia/Kolkata')).toBe('Asia/Kolkata');
  });
});
