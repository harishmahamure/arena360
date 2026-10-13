import type { components } from '@gaming-cafe/api-types';
import { http } from '@gaming-cafe/utils';

export type OnboardingStep = components['schemas']['OnboardingStep'];
export type OnboardingSnapshot = components['schemas']['OnboardingSnapshot'];

export const getOnboarding = () => http.get<OnboardingSnapshot>('/onboarding');
export const saveOnboarding = (input: {
  currentStep: OnboardingStep;
  expectedRevision: number;
  reviewSettings?: boolean;
}) => http.put<OnboardingSnapshot>('/onboarding', input);
export const completeOnboarding = (expectedRevision: number) =>
  http.post<OnboardingSnapshot>('/onboarding/complete', { expectedRevision });
