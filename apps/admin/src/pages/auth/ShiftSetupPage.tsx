import { CurrencyField, FormButton } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import { Alert, Box, CircularProgress, Divider, Typography } from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Navigate, useNavigate } from 'react-router-dom';
import { useSelector } from '../../hooks/store';
import { getShiftStartContext, startShift } from '../../services/shifts';
import { formatDisplayDateTime } from '../../utils/date';

const formatCurrency = (value: number) =>
  new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(value);

export default function ShiftSetupPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const role = useSelector((state) => state.auth.role);
  const [openingBalance, setOpeningBalance] = useState<string>('');
  const [notes, setNotes] = useState('');

  const contextQuery = useQuery({
    queryKey: ['shift-start-context'],
    queryFn: getShiftStartContext,
    retry: false,
  });
  const context = contextQuery.data;
  const resolvedOpening =
    openingBalance === '' ? (context?.suggestedOpeningBalance ?? 0) : Number(openingBalance);

  const startMutation = useMutation({
    mutationFn: () =>
      startShift({
        openingBalance: resolvedOpening,
        notes: notes.trim() || undefined,
      }),
    onSuccess: (result) => {
      void queryClient.invalidateQueries({ queryKey: ['activeShift'] });
      void queryClient.invalidateQueries({ queryKey: ['staffDashboardStats'] });
      toastUtils.success(result.resumed ? 'Shift resumed' : 'Shift and register started');
      navigate('/', { replace: true });
    },
    onError: (error: unknown) => {
      toastUtils.error(error instanceof Error ? error.message : 'Unable to start shift');
    },
  });

  if (!local.get('accessToken')) return <Navigate to="/login" replace />;
  if (role && role !== 'staff') return <Navigate to="/" replace />;

  if (contextQuery.isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (contextQuery.isError || !context) {
    return (
      <Alert severity="error">
        We couldn’t prepare your shift. Refresh the page or contact an administrator.
      </Alert>
    );
  }

  const isResume = context.mode === 'resume';
  return (
    <Box>
      <Typography variant="h4" fontWeight={700} gutterBottom>
        {isResume ? 'Resume your shift' : 'Set up your shift'}
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        {isResume
          ? 'We found an open shift and register for your account.'
          : 'Confirm the opening cash before counter operations are enabled.'}
      </Typography>

      <Alert severity={isResume ? 'info' : 'warning'} sx={{ mb: 3 }}>
        {isResume ? (
          <>
            Started {context.shift ? formatDisplayDateTime(context.shift.clockIn) : 'earlier'} ·
            Opening float {formatCurrency(context.suggestedOpeningBalance)}
          </>
        ) : (
          <>Suggested carry-forward float: {formatCurrency(context.suggestedOpeningBalance)}</>
        )}
      </Alert>

      {!isResume ? (
        <>
          <CurrencyField
            fullWidth
            label="Confirmed opening cash"
            value={openingBalance === '' ? context.suggestedOpeningBalance : openingBalance}
            onChange={(event) => setOpeningBalance(event.target.value)}
            inputProps={{ min: 0 }}
            helperText="Count the drawer and confirm the amount before continuing."
          />
          <Box sx={{ mt: 2 }}>
            <label htmlFor="shift-notes">
              <Typography variant="body2" fontWeight={600} sx={{ mb: 0.75 }}>
                Shift note (optional)
              </Typography>
            </label>
            <Box
              id="shift-notes"
              component="textarea"
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              rows={3}
              sx={{
                width: '100%',
                resize: 'vertical',
                borderRadius: 1,
                border: 1,
                borderColor: 'divider',
                bgcolor: 'background.paper',
                color: 'text.primary',
                p: 1.5,
                font: 'inherit',
              }}
            />
          </Box>
        </>
      ) : null}

      <Divider sx={{ my: 3 }} />
      <FormButton
        fullWidth
        size="large"
        variant="contained"
        disabled={startMutation.isPending || resolvedOpening < 0}
        onClick={() => startMutation.mutate()}
      >
        {startMutation.isPending
          ? 'Preparing workspace…'
          : isResume
            ? 'Resume shift'
            : 'Start shift and open register'}
      </FormButton>
    </Box>
  );
}
