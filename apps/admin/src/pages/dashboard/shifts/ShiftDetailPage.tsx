import { DetailPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Block } from '@mui/icons-material';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Grid,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getPlayerById } from '../../../services/players/getById';
import { forceCloseShift, getShift } from '../../../services/shifts';
import { formatDisplayDateTime, formatDuration } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'warning' }> = {
  active: { label: 'Active', color: 'success' },
  completed: { label: 'Completed', color: 'default' },
  force_closed: { label: 'Force Closed', color: 'warning' },
};

export default function ShiftDetailPage() {
  const { id } = useParams<{ id: string }>();
  const queryClient = useQueryClient();
  const { can } = usePermissions();
  const canForceClose = can(Permission.ShiftsForceClose);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [forcing, setForcing] = useState(false);
  const [error, setError] = useState<string | undefined>();

  const {
    data: shift,
    isLoading,
    error: fetchError,
  } = useQuery({
    queryKey: ['shift', id],
    queryFn: () => getShift(id as string),
    enabled: !!id,
  });

  const { data: staff } = useQuery({
    queryKey: ['player', shift?.userId],
    queryFn: () => getPlayerById(shift?.userId as string),
    enabled: !!shift?.userId,
  });

  const staffName = staff
    ? [staff.firstName, staff.lastName].filter(Boolean).join(' ').trim() || staff.username
    : '—';

  const durationMinutes = shift
    ? (() => {
        const start = new Date(shift.clockIn).getTime();
        const end = shift.clockOut ? new Date(shift.clockOut).getTime() : Date.now();
        return Math.max(0, Math.round((end - start) / 60000));
      })()
    : 0;

  const handleForceClose = async () => {
    if (!id) return;
    setForcing(true);
    setError(undefined);
    try {
      await forceCloseShift(id);
      toastUtils.success('Shift force-closed');
      setConfirmOpen(false);
      void queryClient.invalidateQueries({ queryKey: ['shift', id] });
      void queryClient.invalidateQueries({ queryKey: ['shifts'] });
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to force-close shift');
    } finally {
      setForcing(false);
    }
  };

  const status = shift
    ? (statusConfig[shift.status] ?? { label: 'Unknown', color: 'default' as const })
    : undefined;

  return (
    <>
      <DetailPage
        title="Shift details"
        backTo="/shifts"
        backLabel="Back to shifts"
        breadcrumbs={[{ label: 'Shifts', to: '/shifts' }, { label: 'Shift details' }]}
        isLoading={isLoading}
        error={!isLoading && (fetchError || !shift) ? 'Shift not found' : null}
        onRetry={() => void queryClient.invalidateQueries({ queryKey: ['shift', id] })}
        maxWidth={800}
        status={status}
        banner={
          error ? (
            <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError(undefined)}>
              {error}
            </Alert>
          ) : undefined
        }
        summary={
          shift ? (
            <Grid container spacing={2}>
              <Grid size={{ xs: 12, sm: 6 }}>
                <Typography variant="caption" color="text.secondary">
                  Staff
                </Typography>
                <Typography variant="body1" fontWeight={600}>
                  {staffName}
                </Typography>
              </Grid>
              <Grid size={{ xs: 12, sm: 6 }}>
                <Typography variant="caption" color="text.secondary">
                  Duration
                </Typography>
                <Typography variant="body1" fontWeight={600}>
                  {formatDuration(durationMinutes)}
                </Typography>
              </Grid>
              <Grid size={{ xs: 12, sm: 6 }}>
                <Typography variant="caption" color="text.secondary">
                  Clock in
                </Typography>
                <Typography variant="body1">{formatDisplayDateTime(shift.clockIn)}</Typography>
              </Grid>
              <Grid size={{ xs: 12, sm: 6 }}>
                <Typography variant="caption" color="text.secondary">
                  Clock out
                </Typography>
                <Typography variant="body1">
                  {shift.clockOut ? formatDisplayDateTime(shift.clockOut) : '—'}
                </Typography>
              </Grid>
              {shift.notes && (
                <Grid size={{ xs: 12 }}>
                  <Typography variant="caption" color="text.secondary">
                    Notes
                  </Typography>
                  <Typography variant="body1">{shift.notes}</Typography>
                </Grid>
              )}
            </Grid>
          ) : undefined
        }
        actions={
          canForceClose && shift?.status === 'active' ? (
            <Button
              variant="outlined"
              color="warning"
              startIcon={<Block />}
              onClick={() => setConfirmOpen(true)}
            >
              Force close shift
            </Button>
          ) : undefined
        }
      />

      <Dialog open={confirmOpen} onClose={() => setConfirmOpen(false)}>
        <DialogTitle>Force close shift?</DialogTitle>
        <DialogContent>
          <DialogContentText>
            This will immediately end the active shift for {staffName}. The staff member will need
            to start a new shift before creating sessions or transactions. This action cannot be
            undone.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmOpen(false)} disabled={forcing}>
            Cancel
          </Button>
          <Button
            onClick={() => void handleForceClose()}
            color="warning"
            variant="contained"
            disabled={forcing}
          >
            Force close
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
