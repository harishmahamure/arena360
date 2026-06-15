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
  Typography,
} from '@mui/material';
import { formatDisplayDate } from '../../utils/date';

export interface PlayerPlanDisplay {
  id: string;
  deviceType?: string | null;
  deviceSubType?: string | null;
  remainingMinutes: number;
  expiryDate: string;
  kind: string;
  status: string;
  plan?: { name?: string } | null;
}

function formatDeviceLabel(deviceType?: string | null, deviceSubType?: string | null) {
  if (!deviceType) return '—';
  if (deviceSubType) return `${deviceType} · ${deviceSubType}`;
  return deviceType;
}

function formatKindLabel(kind: string) {
  return kind === 'happy_hours' ? 'Happy Hours' : 'Time Plan';
}

export interface PlayerActivePlansSectionProps {
  plans: PlayerPlanDisplay[];
}

export function PlayerActivePlansSection({ plans }: PlayerActivePlansSectionProps) {
  if (plans.length === 0) {
    return (
      <Typography variant="body2" color="text.secondary">
        No active plans. Purchase a plan to enable kiosk login.
      </Typography>
    );
  }

  return (
    <TableContainer component={Paper} variant="outlined">
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>Plan</TableCell>
            <TableCell>Device</TableCell>
            <TableCell>Remaining</TableCell>
            <TableCell>Expiry</TableCell>
            <TableCell>Kind</TableCell>
            <TableCell>Status</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {plans.map((plan) => (
            <TableRow key={plan.id} hover>
              <TableCell>{plan.plan?.name ?? 'Unknown plan'}</TableCell>
              <TableCell>{formatDeviceLabel(plan.deviceType, plan.deviceSubType)}</TableCell>
              <TableCell>
                <Typography variant="body2" fontWeight={600}>
                  {formatRemainingLabel(plan.remainingMinutes)}
                </Typography>
              </TableCell>
              <TableCell>{formatDisplayDate(plan.expiryDate)}</TableCell>
              <TableCell>
                <Chip label={formatKindLabel(plan.kind)} size="small" color="primary" />
              </TableCell>
              <TableCell>
                <Chip label={plan.status} size="small" color="success" />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
}
