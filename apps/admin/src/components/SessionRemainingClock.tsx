import type { DeductionProfile } from '@gaming-cafe/contracts';
import { DEFAULT_CAFE_TZ } from '@gaming-cafe/contracts';
import { formatRemainingLabel, useSessionRemainingMinutes } from '@gaming-cafe/utils';
import { Typography } from '@mui/material';

interface SessionRemainingClockProps {
  sessionStartTime: string;
  remainingMinutes: number;
  timeCreditsConsumed?: number | null;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  variant?: 'default' | 'prominent';
}

/**
 * Display-only session countdown shared with the kiosk HUD.
 * Ticks locally from session start using backend-aligned weighted consumption.
 */
export function SessionRemainingClock({
  sessionStartTime,
  remainingMinutes,
  timeCreditsConsumed = 0,
  deductionProfile,
  cafeTimezone = DEFAULT_CAFE_TZ,
  variant = 'default',
}: SessionRemainingClockProps) {
  const localMinutes = useSessionRemainingMinutes({
    sessionStartTime,
    walletBalanceMinutes: remainingMinutes,
    timeCreditsConsumed,
    deductionProfile,
    cafeTimezone,
  });
  const isProminent = variant === 'prominent';

  if (localMinutes === null || localMinutes <= 0) {
    return (
      <Typography
        variant={isProminent ? 'h4' : 'body2'}
        fontWeight={isProminent ? 700 : undefined}
        color="error"
      >
        Expired
      </Typography>
    );
  }

  return (
    <Typography
      variant={isProminent ? 'h4' : 'body2'}
      fontWeight={isProminent ? 700 : undefined}
      color={isProminent ? 'primary.main' : undefined}
    >
      {formatRemainingLabel(localMinutes)}
    </Typography>
  );
}
