/**
 * Raw wallet minutes from API responses. Never use effective `remainingMinutes`
 * here — that value is already time-adjusted and causes double subtraction in
 * the session countdown.
 */
export function walletMinutesFromResponse(
  walletBalanceMinutes: number | undefined,
  remainingMinutes?: number,
): number {
  if (typeof walletBalanceMinutes === 'number') return walletBalanceMinutes;
  if (typeof remainingMinutes === 'number') return remainingMinutes;
  return 0;
}
