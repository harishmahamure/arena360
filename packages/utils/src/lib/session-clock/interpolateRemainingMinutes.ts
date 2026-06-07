import { currentDeductionRatio, type DeductionProfile } from '@gaming-cafe/contracts';

/** Wallet minutes left after local interpolation from a server anchor. */
export function interpolateRemainingMinutes(
  authoritativeMinutes: number,
  syncedAtMs: number,
  nowMs: number,
  deductionProfile?: DeductionProfile | null,
  cafeTimezone?: string,
): number {
  const elapsedWallMinutes = (nowMs - syncedAtMs) / 60_000;
  const ratio =
    deductionProfile && cafeTimezone
      ? currentDeductionRatio(deductionProfile, cafeTimezone, new Date(nowMs))
      : 1;
  return Math.max(0, authoritativeMinutes - elapsedWallMinutes * ratio);
}
