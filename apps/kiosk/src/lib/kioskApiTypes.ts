import type { DeductionProfile } from '@gaming-cafe/contracts';

/**
 * Kiosk HTTP DTOs aligned with `packages/api-types` OpenAPI shapes.
 * Kept local to avoid pulling the full generated schema into the kiosk bundle
 * until upstream duplicate-operation-id issues are resolved.
 */
export interface KioskSessionResponseDto {
  sessionId: string;
  balanceId: string;
  deviceId: string;
  startTime: string;
  remainingMinutes: number;
  walletBalanceMinutes?: number;
  resumed: boolean;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string;
}

export interface ActiveSessionDto {
  id: string;
  startTime: string;
  balanceId: string;
  remainingMinutes?: number;
  walletBalanceMinutes?: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  timeCreditsConsumed?: number | null;
  expiryDate?: string;
}

export interface ProvisionDeviceDto {
  fingerprint: {
    mac: string;
    serial: string;
    biosUuid: string;
    platform: string;
    collectedAt: string;
  };
  name: string;
  deviceType?: string | null;
  deviceSubType?: string | null;
  location?: string | null;
  provisionClient?: string | null;
}
