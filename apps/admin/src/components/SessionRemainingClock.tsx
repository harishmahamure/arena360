import type { DeductionProfile } from '@gaming-cafe/contracts';
import { DEFAULT_CAFE_TZ } from '@gaming-cafe/contracts';
import { formatRemainingLabel, useSessionRemainingMinutes } from '@gaming-cafe/utils';
import { Typography } from '@mui/material';

interface SessionRemainingClockProps {
  remainingMinutes: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  variant?: 'default' | 'prominent';
}

/**
 * Display-only session countdown shared with the kiosk HUD.
 * Server balance is authoritative; this interpolates between list refetches.
 */
export function SessionRemainingClock({
  remainingMinutes,
  deductionProfile,
  cafeTimezone = DEFAULT_CAFE_TZ,
  variant = 'default',
}: SessionRemainingClockProps) {
  const localMinutes = useSessionRemainingMinutes(remainingMinutes, deductionProfile, cafeTimezone);
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
