import type { PricingPolicy } from '../../../services/pricing-rules';

/** Keep malformed advanced drafts out of both the visual editor and API requests. */
export function parsePricingPolicy(text: string): PricingPolicy {
  const policy = JSON.parse(text) as PricingPolicy | null;
  if (!policy || typeof policy.baseRate !== 'string' || !Array.isArray(policy.rules))
    throw new Error('A policy needs a base rate and a rules array.');
  const decimal = (value: unknown) =>
    typeof value === 'string' &&
    value.trim() !== '' &&
    Number.isFinite(Number(value)) &&
    Number(value) >= 0;
  if (!decimal(policy.baseRate)) throw new Error('Base rate must be a non-negative number.');
  if (
    !Number.isInteger(policy.roundingScale) ||
    policy.roundingScale < 0 ||
    policy.roundingScale > 4
  )
    throw new Error('Decimal places must be between 0 and 4.');
  for (const rule of policy.rules) {
    if (
      !rule ||
      typeof rule.id !== 'string' ||
      typeof rule.name !== 'string' ||
      !Array.isArray(rule.deviceTypes) ||
      !rule.deviceTypes.every((type) => typeof type === 'string') ||
      !Array.isArray(rule.weekdays) ||
      !rule.weekdays.every((day) => Number.isInteger(day) && day >= 1 && day <= 7) ||
      !Number.isInteger(rule.priority) ||
      !rule.action ||
      !['fixed', 'multiplier'].includes(rule.action.type) ||
      !decimal(rule.action.value)
    )
      throw new Error(
        'Each rule needs a name, ID, priority, device types, weekdays, and a valid adjustment.',
      );
    if (
      (rule.startTime != null && typeof rule.startTime !== 'string') ||
      (rule.endTime != null && typeof rule.endTime !== 'string')
    )
      throw new Error('Rule times must be text values.');
  }
  return policy;
}
