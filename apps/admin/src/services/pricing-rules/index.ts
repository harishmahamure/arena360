import { http } from '@gaming-cafe/utils';

export interface PricingRule {
  id: string;
  name: string;
  priority: number;
  deviceTypes: string[];
  weekdays: number[];
  startTime?: string | null;
  endTime?: string | null;
  startsAt?: string | null;
  endsAt?: string | null;
  action: { type: 'fixed' | 'multiplier'; value: string };
}

export interface PricingPolicy {
  baseRate: string;
  rules: PricingRule[];
  roundingScale: number;
  minimumPrice?: string | null;
  maximumPrice?: string | null;
}

export interface PricingRuleSet {
  id: string;
  organizationId: string;
  locationId?: string | null;
  name: string;
  description?: string | null;
  activeVersionId?: string | null;
}

export interface PricingRuleVersion {
  id: string;
  ruleSetId: string;
  version: number;
  status: 'draft' | 'validated' | 'scheduled' | 'published' | 'superseded';
  policy: PricingPolicy;
  simulationHash?: string | null;
  effectiveAt?: string | null;
  publishedAt?: string | null;
  createdAt: string;
}

export interface PricingSimulationResult {
  baseRate: string;
  finalPrice: string;
  currency: string;
  timezone: string;
  simulationHash: string;
  trace: Array<{
    ruleId: string;
    ruleName: string;
    action: string;
    before: string;
    after: string;
  }>;
}

export const listPricingRuleSets = (organizationId: string, locationId?: string) =>
  http.get<PricingRuleSet[]>(`/organizations/${organizationId}/pricing-rule-sets`, {
    params: locationId ? { locationId } : {},
  });

export const createPricingRuleSet = (
  organizationId: string,
  input: { locationId?: string; name: string; description?: string; policy: PricingPolicy },
) =>
  http.post<{ ruleSet: PricingRuleSet; version: PricingRuleVersion }>(
    `/organizations/${organizationId}/pricing-rule-sets`,
    input,
  );

export const listPricingRuleVersions = (organizationId: string, setId: string) =>
  http.get<PricingRuleVersion[]>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions`,
  );

export const createPricingRuleVersion = (
  organizationId: string,
  setId: string,
  policy: PricingPolicy,
) =>
  http.post<PricingRuleVersion>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions`,
    { policy },
  );

export const validatePricingRuleVersion = (
  organizationId: string,
  setId: string,
  versionId: string,
) =>
  http.post<PricingRuleVersion>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions/${versionId}/validate`,
  );

export const simulatePricingRuleVersion = (
  organizationId: string,
  setId: string,
  versionId: string,
  input: { locationId?: string; deviceType?: string; at: string; baseRate?: string },
) =>
  http.post<PricingSimulationResult>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions/${versionId}/simulate`,
    input,
  );

export const publishPricingRuleVersion = (
  organizationId: string,
  setId: string,
  versionId: string,
  effectiveAt?: string,
) =>
  http.post<PricingRuleVersion>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions/${versionId}/publish`,
    { effectiveAt },
  );

export const rollbackPricingRuleVersion = (
  organizationId: string,
  setId: string,
  versionId: string,
) =>
  http.post<PricingRuleVersion>(
    `/organizations/${organizationId}/pricing-rule-sets/${setId}/versions/${versionId}/rollback`,
  );
