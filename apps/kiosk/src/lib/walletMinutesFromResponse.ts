/**
 * Raw wallet minutes from API responses. Never use effective `remainingMinutes`
 * here — that value is already time-adjusted and causes double subtraction in
 * the session countdown.
 */
export function walletMinutesFromResponse(walletBalanceMinutes: number | undefined): number {
  return typeof walletBalanceMinutes === 'number' ? walletBalanceMinutes : 0;
}
