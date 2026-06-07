import {
  type DeductionProfile,
  maxWallMinutes,
  validateDeductionProfile,
  windowsOverlap,
} from '@gaming-cafe/contracts';
import {
  Alert,
  Box,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';

interface DeductionPreviewProps {
  timeCredits: number;
  dynamicDeductionEnabled?: boolean;
  peakWindowStart?: string;
  peakWindowEnd?: string;
  peakRatio?: number;
  lowWindowStart?: string;
  lowWindowEnd?: string;
  lowRatio?: number;
}

function buildProfile(props: DeductionPreviewProps): DeductionProfile | null {
  if (!props.dynamicDeductionEnabled) return null;
  const profile: DeductionProfile = {
    peakWindowStart: props.peakWindowStart ?? '',
    peakWindowEnd: props.peakWindowEnd ?? '',
    peakRatio: props.peakRatio ?? 0,
    lowWindowStart: props.lowWindowStart ?? '',
    lowWindowEnd: props.lowWindowEnd ?? '',
    lowRatio: props.lowRatio ?? 0,
  };
  return profile;
}

export function DeductionPreview(props: DeductionPreviewProps) {
  if (!props.dynamicDeductionEnabled) return null;

  const profile = buildProfile(props);
  if (!profile) return null;

  const validationError = validateDeductionProfile(profile);
  const overlap = !validationError && windowsOverlap(profile);
  const credits = props.timeCredits > 0 ? props.timeCredits : 0;

  const rows = [
    {
      period: 'Low hours',
      ratio: profile.lowRatio,
      wall: credits > 0 ? maxWallMinutes(credits, profile.lowRatio) : 0,
    },
    {
      period: 'Normal hours',
      ratio: 1,
      wall: credits,
    },
    {
      period: 'Peak hours',
      ratio: profile.peakRatio,
      wall: credits > 0 ? maxWallMinutes(credits, profile.peakRatio) : 0,
    },
  ];

  return (
    <Box sx={{ mt: 1 }}>
      <Typography variant="subtitle2" gutterBottom>
        Deduction preview
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
        Player receives full wallet minutes. Burn rate varies by local time window. Mixed sessions
        use weighted totals.
      </Typography>
      {validationError ? (
        <Alert severity="warning" sx={{ mb: 1 }}>
          {validationError}
        </Alert>
      ) : null}
      {overlap ? (
        <Alert severity="error" sx={{ mb: 1 }}>
          Peak and low windows overlap.
        </Alert>
      ) : null}
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Period</TableCell>
            <TableCell>Ratio</TableCell>
            <TableCell>Max play (single window)</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => (
            <TableRow key={row.period}>
              <TableCell>{row.period}</TableCell>
              <TableCell>{row.ratio}×</TableCell>
              <TableCell>
                {credits > 0 ? `${credits} ÷ ${row.ratio} = ${row.wall.toFixed(1)} min` : '—'}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
}
