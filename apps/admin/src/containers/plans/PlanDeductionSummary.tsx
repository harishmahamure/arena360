import {
  buildDeductionPlayBreakdown,
  type DeductionPlayPeriod,
  type DeductionPlayRow,
  type DeductionProfile,
} from '@gaming-cafe/contracts';
import { Alert, Box, Typography } from '@mui/material';

export interface PlanDeductionSummaryProps {
  timeCredits: number;
  deductionProfile: DeductionProfile;
}

function periodShortLabel(period: DeductionPlayPeriod): string {
  if (period === 'peak') return 'Peak';
  if (period === 'low') return 'Low';
  return 'Other hours';
}

function burnRateHint(row: DeductionPlayRow): string {
  if (row.period === 'peak') return `burns ${row.ratio}× faster`;
  if (row.period === 'low') return `burns ${row.ratio}× slower`;
  return 'normal 1×';
}

function formatPlayLine(row: DeductionPlayRow): string {
  const play =
    row.walletMinutes > 0 ? `~${row.wallPlayMinutes.toFixed(1)} min play` : 'no play time';
  return `${periodShortLabel(row.period)} ${row.timeRange}: ${play} (${burnRateHint(row)})`;
}

export function PlanDeductionSummary({
  timeCredits,
  deductionProfile,
}: PlanDeductionSummaryProps) {
  const credits = timeCredits > 0 ? timeCredits : 0;
  const rows = buildDeductionPlayBreakdown(credits, deductionProfile);

  return (
    <Alert severity="info" sx={{ mt: 2 }}>
      <Typography variant="body2" sx={{ mb: 1 }}>
        Gets <strong>{credits} wallet minutes</strong> added to balance (use anytime within
        validity).
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
        Play time if used entirely in one period (cafe local time):
      </Typography>
      <Box component="ul" sx={{ m: 0, pl: 2.5 }}>
        {rows.map((row) => (
          <Typography key={row.period} component="li" variant="body2" sx={{ mb: 0.25 }}>
            {formatPlayLine(row)}
          </Typography>
        ))}
      </Box>
      <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>
        Mixed sessions deduct proportionally across windows.
      </Typography>
    </Alert>
  );
}
