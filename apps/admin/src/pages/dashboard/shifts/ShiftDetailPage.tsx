import { toastUtils } from '@gaming-cafe/utils';
import { ArrowBack, Block } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  GridLegacy as Grid,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { usePermissions } from '../../../hooks/usePermissions';
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
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { isAdmin } = usePermissions();
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

  if (isLoading) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>Loading shift…</Typography>
      </Box>
    );
  }

  if (fetchError || !shift) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error">Shift not found</Alert>
        <Button startIcon={<ArrowBack />} onClick={() => navigate('/shifts')} sx={{ mt: 2 }}>
          Back to shifts
        </Button>
      </Box>
    );
  }

  const status = statusConfig[shift.status] ?? {
    label: 'Unknown',
    color: 'default' as const,
  };

  return (
    <Box sx={{ px: 4, py: 3, maxWidth: 800 }}>
      <Button startIcon={<ArrowBack />} onClick={() => navigate('/shifts')} sx={{ mb: 2 }}>
        Back to shifts
      </Button>

      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 3 }}>
        <Typography variant="h4" fontWeight={600}>
          Shift Details
        </Typography>
        <Chip label={status.label} color={status.color} size="small" />
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Card variant="outlined" sx={{ mb: 3 }}>
        <CardContent>
          <Grid container spacing={2}>
            <Grid item xs={12} sm={6}>
              <Typography variant="caption" color="text.secondary">
                Staff
              </Typography>
              <Typography variant="body1" fontWeight={600}>
                {staffName}
              </Typography>
            </Grid>
            <Grid item xs={12} sm={6}>
              <Typography variant="caption" color="text.secondary">
                Duration
              </Typography>
              <Typography variant="body1" fontWeight={600}>
                {formatDuration(durationMinutes)}
              </Typography>
            </Grid>
            <Grid item xs={12} sm={6}>
              <Typography variant="caption" color="text.secondary">
                Clock in
              </Typography>
              <Typography variant="body1">{formatDisplayDateTime(shift.clockIn)}</Typography>
            </Grid>
            <Grid item xs={12} sm={6}>
              <Typography variant="caption" color="text.secondary">
                Clock out
              </Typography>
              <Typography variant="body1">
                {shift.clockOut ? formatDisplayDateTime(shift.clockOut) : '—'}
              </Typography>
            </Grid>
            {shift.notes && (
              <Grid item xs={12}>
                <Typography variant="caption" color="text.secondary">
                  Notes
                </Typography>
                <Typography variant="body1">{shift.notes}</Typography>
              </Grid>
            )}
          </Grid>
        </CardContent>
      </Card>

      {isAdmin && shift.status === 'active' && (
        <Stack direction="row" spacing={2}>
          <Button
            variant="outlined"
            color="warning"
            startIcon={<Block />}
            onClick={() => setConfirmOpen(true)}
          >
            Force close shift
          </Button>
        </Stack>
      )}

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
    </Box>
  );
}
