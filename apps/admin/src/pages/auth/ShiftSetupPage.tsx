import { CurrencyField, FormButton } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import { Alert, Box, Button, CircularProgress, Typography } from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Navigate, useNavigate } from 'react-router-dom';
import { GuidedForm, GuidedStep } from '../../components/GuidedForm';
import LocationChoice from '../../components/LocationChoice';
import { usePermissions } from '../../hooks/usePermissions';
import { rememberLocation } from '../../lib/locationSelection';
import { getVenueLocations } from '../../services/config';
import { getShiftStartContext, startShift } from '../../services/shifts';
import { formatDisplayDateTime } from '../../utils/date';
import { currentOrganizationId } from '../dashboard/access/LocationsPanel';

export default function ShiftSetupPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { isStaff } = usePermissions();
  const [openingBalance, setOpeningBalance] = useState<string>('');
  const [notes, setNotes] = useState('');
  const [selectedVenueId, setSelectedVenueId] = useState('');
  const venuesQuery = useQuery({
    queryKey: ['shift-venues', currentOrganizationId()],
    queryFn: () => getVenueLocations(currentOrganizationId()),
    enabled: Boolean(local.get('accessToken') && isStaff),
  });
  const venues = venuesQuery.data?.filter((venue) => venue.isActive) ?? [];
  const venueId =
    venues.find((venue) => venue.id === selectedVenueId)?.id ||
    (venues.length === 1 ? venues[0]?.id : undefined);
  const venue = venues.find((item) => item.id === venueId);
  const formatCurrency = (value: number) =>
    new Intl.NumberFormat('en-IN', {
      style: 'currency',
      currency: venue?.currency ?? 'INR',
    }).format(value);

  const contextQuery = useQuery({
    queryKey: ['shift-start-context', venueId],
    queryFn: () => getShiftStartContext(venueId),
    enabled: Boolean(local.get('accessToken') && isStaff && venueId),
    retry: false,
  });
  const context = contextQuery.data;
  const resolvedOpening =
    openingBalance === '' ? (context?.suggestedOpeningBalance ?? 0) : Number(openingBalance);

  const startMutation = useMutation({
    mutationFn: (locationId: string) => {
      if (!venues.some((venue) => venue.id === locationId))
        throw new Error('Choose a location before starting your shift.');
      return startShift({
        openingBalance: resolvedOpening,
        notes: notes.trim() || undefined,
        venueLocationId: locationId,
      });
    },
    onSuccess: (result, locationId) => {
      rememberLocation(locationId);
      // A new operating location must not reuse dashboard or counter data from another venue.
      queryClient.clear();
      toastUtils.success(result.resumed ? 'Shift resumed' : 'Shift and register started');
      navigate('/', { replace: true });
    },
    onError: (error: unknown) => {
      toastUtils.error(error instanceof Error ? error.message : 'Unable to start shift');
    },
  });

  if (!local.get('accessToken')) return <Navigate to="/login" replace />;
  if (!isStaff) return <Navigate to="/" replace />;

  if (venuesQuery.isLoading || contextQuery.isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (venuesQuery.isError || !venues.length) {
    return (
      <Alert
        severity="error"
        action={
          venuesQuery.isError && <Button onClick={() => void venuesQuery.refetch()}>Retry</Button>
        }
      >
        {venuesQuery.isError
          ? 'Locations could not be loaded. Please try again.'
          : 'No available location for this shift. Ask an administrator to assign one.'}
      </Alert>
    );
  }

  if (!venueId)
    return (
      <LocationChoice
        locations={venues}
        onChoose={setSelectedVenueId}
        title="Choose your shift location"
        description="Select the venue where you will open or resume your shift. We’ll load the opening cash details for that location."
        action="Continue to shift setup"
      />
    );

  const venueSelector = (
    <Box sx={{ mb: 3 }}>
      <Typography fontWeight={600}>{venue?.name}</Typography>
      {venues.length > 1 && (
        <Button
          disabled={startMutation.isPending}
          onClick={() => {
            setSelectedVenueId('');
            setOpeningBalance('');
            setNotes('');
          }}
        >
          Choose another location
        </Button>
      )}
    </Box>
  );

  if (contextQuery.isError || !context) {
    return (
      <Box>
        {venueSelector}
        <Alert
          severity="error"
          action={
            <Button disabled={contextQuery.isFetching} onClick={() => void contextQuery.refetch()}>
              Retry
            </Button>
          }
        >
          We couldn’t prepare your shift at this location. Select another location or contact an
          administrator. {contextQuery.error?.message}
        </Alert>
      </Box>
    );
  }

  const isResume = context.mode === 'resume';
  return (
    <Box>
      {venueSelector}
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

      {isResume ? (
        <FormButton
          fullWidth
          size="large"
          variant="contained"
          disabled={
            startMutation.isPending || !Number.isFinite(resolvedOpening) || resolvedOpening < 0
          }
          onClick={() => startMutation.mutate(venueId)}
        >
          {startMutation.isPending
            ? 'Preparing workspace…'
            : isResume
              ? 'Resume shift'
              : 'Start shift and open register'}
        </FormButton>
      ) : (
        <GuidedForm
          busy={startMutation.isPending}
          actions={
            <FormButton
              fullWidth
              size="large"
              variant="contained"
              disabled={
                startMutation.isPending || !Number.isFinite(resolvedOpening) || resolvedOpening < 0
              }
              onClick={() => startMutation.mutate(venueId)}
            >
              {startMutation.isPending
                ? 'Preparing workspace…'
                : isResume
                  ? 'Resume shift'
                  : 'Start shift and open register'}
            </FormButton>
          }
        >
          <GuidedStep
            title="Opening cash & notes"
            validate={() =>
              !Number.isFinite(resolvedOpening) || resolvedOpening < 0
                ? 'Enter a nonnegative cash count.'
                : undefined
            }
          >
            {' '}
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
          </GuidedStep>
        </GuidedForm>
      )}
    </Box>
  );
}
