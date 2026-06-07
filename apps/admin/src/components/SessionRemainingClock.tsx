import type { DeductionProfile } from '@gaming-cafe/contracts';
import { DEFAULT_CAFE_TZ } from '@gaming-cafe/contracts';
import { formatRemainingLabel, useSessionRemainingMinutes } from '@gaming-cafe/utils';
import { Typography } from '@mui/material';

interface SessionRemainingClockProps {
  remainingMinutes: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
}

/**
 * Display-only session countdown shared with the kiosk HUD.
 * Server balance is authoritative; this interpolates between list refetches.
 */
export function SessionRemainingClock({
  remainingMinutes,
  deductionProfile,
  cafeTimezone = DEFAULT_CAFE_TZ,
}: SessionRemainingClockProps) {
  const localMinutes = useSessionRemainingMinutes(remainingMinutes, deductionProfile, cafeTimezone);

  if (localMinutes === null || localMinutes <= 0) {
    return (
      <Typography variant="body2" color="error">
        Expired
      </Typography>
    );
  }

  return <Typography variant="body2">{formatRemainingLabel(localMinutes)}</Typography>;
}
