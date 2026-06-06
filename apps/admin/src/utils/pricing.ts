const DEFAULT_CAFE_TZ = 'Asia/Kolkata';

/** Night pricing window: 23:00–08:00 in venue local time. */
export function isNightPricingWindow(now = new Date(), timeZone = DEFAULT_CAFE_TZ): boolean {
  const hour = Number(
    new Intl.DateTimeFormat('en-GB', {
      timeZone,
      hour: 'numeric',
      hour12: false,
    }).format(now),
  );
  return hour >= 23 || hour < 8;
}

export function effectiveProductPrice(
  dayPrice: number,
  nightPrice: number,
  now = new Date(),
  timeZone = DEFAULT_CAFE_TZ,
): number {
  return isNightPricingWindow(now, timeZone) ? nightPrice : dayPrice;
}
