import { formatRemainingLabel } from '@gaming-cafe/utils';
import {
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
} from '@mui/material';
import { formatDisplayDate, formatDisplayDateTime } from '../../utils/date';

export interface PlayerPlanDisplay {
  id: string;
  deviceType?: string | null;
  deviceSubType?: string | null;
  remainingMinutes: number;
  expiryDate: string;
  status: string;
  updatedAt: string;
  plan?: { name?: string } | null;
}

function formatDeviceLabel(deviceType?: string | null, deviceSubType?: string | null) {
  if (!deviceType) return '—';
  if (deviceSubType) return `${deviceType} · ${deviceSubType}`;
  return deviceType;
}

export interface PlayerExhaustedPlansSectionProps {
  plans: PlayerPlanDisplay[];
}

export function PlayerExhaustedPlansSection({ plans }: PlayerExhaustedPlansSectionProps) {
  if (plans.length === 0) {
    return null;
  }

  return (
    <TableContainer component={Paper} variant="outlined" sx={{ opacity: 0.92 }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Plan</TableCell>
            <TableCell>Device</TableCell>
            <TableCell>Remaining</TableCell>
            <TableCell>Exhausted on</TableCell>
            <TableCell>Original expiry</TableCell>
            <TableCell>Status</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {plans.map((plan) => (
            <TableRow key={plan.id}>
              <TableCell>{plan.plan?.name ?? 'Unknown plan'}</TableCell>
              <TableCell>{formatDeviceLabel(plan.deviceType, plan.deviceSubType)}</TableCell>
              <TableCell>
                {plan.remainingMinutes > 0 ? formatRemainingLabel(plan.remainingMinutes) : '0 min'}
              </TableCell>
              <TableCell>{formatDisplayDateTime(plan.updatedAt)}</TableCell>
              <TableCell>{formatDisplayDate(plan.expiryDate)}</TableCell>
              <TableCell>
                <Chip label="exhausted" size="small" color="default" />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
}
