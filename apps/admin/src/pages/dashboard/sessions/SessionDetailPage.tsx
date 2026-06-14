import { DEFAULT_CAFE_TZ } from '@gaming-cafe/contracts';
import { DetailPage, type DetailPageSection, type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { formatDate, formatRemainingLabel } from '@gaming-cafe/utils';
import { Stop, Timer } from '@mui/icons-material';
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
  Divider,
  Grid,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { type ReactNode, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { SessionRemainingClock } from '../../../components/SessionRemainingClock';
import { StaffTotpDialog } from '../../../components/StaffTotpDialog';
import {
  type EndSessionFormData,
  endSessionSchema,
} from '../../../containers/sessions/schemas/session-schema';
import { useSelector } from '../../../hooks/store';
import { getDeviceById } from '../../../services/devices/getById';
import { getPlanById } from '../../../services/plans/getById';
import { getPlayerPlanById } from '../../../services/player-plans/getById';
import { getPlayerById } from '../../../services/players/getById';
import { getSessionById } from '../../../services/sessions/getById';
import { endSession, forceEndSession } from '../../../services/sessions/update';

const formatDuration = (minutes?: number | null) => {
  if (!minutes) return 'N/A';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
};

function isConsoleTvDevice(deviceType?: string) {
  return deviceType === 'PS5' || deviceType === 'PS4';
}

function forceEndDialogMessage(deviceType?: string) {
  if (isConsoleTvDevice(deviceType)) {
    return 'The PlayStation station will end the session immediately and return to the idle screen. Use this for stuck or unattended stations.';
  }
  return "The player's kiosk will show a 5-minute grace warning and then lock and close their apps. Use this for stuck or unattended stations.";
}

function forceEndSuccessMessage(deviceType?: string) {
  if (isConsoleTvDevice(deviceType)) {
    return 'Session force-ended. The PlayStation station has been notified.';
  }
  return 'Session force-ended. The kiosk has been notified.';
}

const endSessionFormFields: FieldConfig<EndSessionFormData>[] = [
  {
    name: 'endTime',
    label: 'End Time (Optional)',
    type: 'datetime',
    placeholder: 'Leave empty to use current time',
    fullWidth: true,
    helperText: 'Optional: Specify a custom end time or leave empty for current time',
  },
];

function DetailField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Grid size={{ xs: 12, md: 6 }}>
      <Typography variant="caption" color="text.secondary" display="block">
        {label}
      </Typography>
      {children}
    </Grid>
  );
}

function TimelineCard({ children }: { children: ReactNode }) {
  return (
    <Card
      variant="outlined"
      sx={{
        borderLeft: 3,
        borderLeftColor: 'primary.main',
      }}
    >
      <CardContent>{children}</CardContent>
    </Card>
  );
}

function SessionDetailActionBar({
  onBuyMoreTime,
  onEndSession,
  onForceEnd,
}: {
  onBuyMoreTime: () => void;
  onEndSession: () => void;
  onForceEnd: () => void;
}) {
  return (
    <Box
      sx={{
        display: { xs: 'flex', md: 'none' },
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 0,
        p: 2,
        gap: 1,
        flexDirection: 'column',
        bgcolor: 'background.paper',
        borderTop: 1,
        borderColor: 'divider',
        zIndex: (theme) => theme.zIndex.appBar - 1,
      }}
    >
      <Button
        variant="contained"
        fullWidth
        startIcon={<Timer />}
        onClick={onBuyMoreTime}
        sx={{ minHeight: 44 }}
      >
        Buy more time
      </Button>
      <Stack direction="row" spacing={1}>
        <Button variant="outlined" fullWidth onClick={onEndSession} sx={{ minHeight: 44 }}>
          End session
        </Button>
        <Button variant="text" color="error" fullWidth onClick={onForceEnd} sx={{ minHeight: 44 }}>
          Force-end
        </Button>
      </Stack>
    </Box>
  );
}

export default function ViewSessionPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [confirmForce, setConfirmForce] = useState(false);
  const [isForcing, setIsForcing] = useState(false);
  const [totpDialog, setTotpDialog] = useState<{
    open: boolean;
    mode: 'end' | 'force';
    loading: boolean;
  }>({ open: false, mode: 'end', loading: false });
  const [pendingEndData, setPendingEndData] = useState<EndSessionFormData | null>(null);
  const currentUserRole = useSelector((state) => state.auth.role);

  const {
    data: session,
    isLoading,
    error: fetchError,
    refetch,
  } = useQuery({
    queryKey: ['session', id],
    queryFn: () => getSessionById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
    refetchInterval: () => false,
  });

  const { data: balanceRecord } = useQuery({
    queryKey: ['player-plan', session?.balanceId],
    queryFn: () => {
      const balanceId = session?.balanceId;
      if (!balanceId) {
        throw new Error('Missing balance ID');
      }
      return getPlayerPlanById(balanceId);
    },
    enabled:
      !!session?.balanceId && (!session?.balance || session.balance.deductionProfile == null),
  });

  const { data: deviceRecord } = useQuery({
    queryKey: ['device', session?.deviceId],
    queryFn: () => {
      const deviceId = session?.deviceId;
      if (!deviceId) {
        throw new Error('Missing device ID');
      }
      return getDeviceById(deviceId);
    },
    enabled: !!session?.deviceId && !session?.device,
  });

  const playerId = session?.balance?.playerId ?? balanceRecord?.playerId;
  const planId = session?.balance?.plan?.id ?? balanceRecord?.sourcePlanId;

  const { data: playerRecord } = useQuery({
    queryKey: ['player', playerId],
    queryFn: () => {
      if (!playerId) {
        throw new Error('Missing player ID');
      }
      return getPlayerById(playerId);
    },
    enabled: !!playerId && !session?.balance?.player,
  });

  const { data: planRecord } = useQuery({
    queryKey: ['plan', planId],
    queryFn: () => {
      if (!planId) {
        throw new Error('Missing plan ID');
      }
      return getPlanById(planId);
    },
    enabled: !!planId && !session?.balance?.plan,
  });

  const startTime = session?.startTime;
  const endTime = session?.endTime;
  const durationMinutes = session?.durationMinutes;
  const timeCreditsConsumed = session?.timeCreditsConsumed;
  const createdAt = session?.createdAt;
  const updatedAt = session?.updatedAt;
  const cafeTimezone = session?.cafeTimezone ?? DEFAULT_CAFE_TZ;
  const balance =
    session?.balance ??
    (balanceRecord
      ? {
          ...balanceRecord,
          player: playerRecord,
          plan: planRecord,
        }
      : undefined);
  const device = session?.device ?? deviceRecord;

  const isActive = !endTime;

  const submitEndSession = async (data: EndSessionFormData, staffTotp?: string) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await endSession(id as string, {
        endTime: data.endTime || undefined,
        staffTotp,
      });
      setSuccess('Session ended successfully!');
      await refetch();
      setTimeout(() => navigate('/sessions'), 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to end session');
      throw err;
    } finally {
      setIsSubmitting(false);
    }
  };

  const submitForceEnd = async (staffTotp?: string) => {
    setIsForcing(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await forceEndSession(id as string, staffTotp);
      setConfirmForce(false);
      setSuccess(forceEndSuccessMessage(device?.deviceType));
      await refetch();
      setTimeout(() => navigate('/sessions'), 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to force-end session');
      throw err;
    } finally {
      setIsForcing(false);
    }
  };

  const handleEndSession = async (data: EndSessionFormData) => {
    if (currentUserRole === 'staff') {
      setPendingEndData(data);
      setTotpDialog({ open: true, mode: 'end', loading: false });
      return;
    }
    await submitEndSession(data);
  };

  const handleForceEndConfirm = () => {
    setConfirmForce(false);
    if (currentUserRole === 'staff') {
      setTotpDialog({ open: true, mode: 'force', loading: false });
      return;
    }
    void submitForceEnd();
  };

  const handleTotpConfirm = async (staffTotp: string) => {
    setTotpDialog((prev) => ({ ...prev, loading: true }));
    try {
      if (totpDialog.mode === 'force') {
        await submitForceEnd(staffTotp);
      } else if (pendingEndData) {
        await submitEndSession(pendingEndData, staffTotp);
      }
      setTotpDialog({ open: false, mode: 'end', loading: false });
      setPendingEndData(null);
    } catch {
      setTotpDialog((prev) => ({ ...prev, loading: false }));
    }
  };

  const handleCancel = () => {
    navigate('/sessions');
  };

  const handleBuyMoreTime = () => {
    if (playerId) {
      navigate(`/plan-transactions/new?playerId=${playerId}`);
    } else {
      navigate('/plan-transactions/new');
    }
  };

  const scrollToEndSession = () => {
    document.getElementById('end-session')?.scrollIntoView({ behavior: 'smooth' });
  };

  const summary = useMemo(() => {
    if (!session) return undefined;

    return (
      <Grid container spacing={2} alignItems="center">
        <Grid size={{ xs: 12, sm: isActive && balance?.remainingMinutes != null ? 6 : 12 }}>
          <Typography variant="h5" fontWeight={600}>
            {balance?.player?.username || 'Unknown player'}
          </Typography>
          <Typography variant="subtitle1" color="text.secondary">
            {device?.name || 'Unknown device'}
          </Typography>
          {startTime && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              Started {formatDate(startTime, 'datetime')}
            </Typography>
          )}
        </Grid>
        {isActive && balance?.remainingMinutes != null && (
          <Grid size={{ xs: 12, sm: 6 }}>
            <Typography variant="caption" color="text.secondary" display="block">
              Time remaining
            </Typography>
            <SessionRemainingClock
              variant="prominent"
              sessionStartTime={startTime ?? session.startTime}
              remainingMinutes={balance.remainingMinutes}
              timeCreditsConsumed={session.timeCreditsConsumed}
              deductionProfile={balance.deductionProfile ?? balanceRecord?.deductionProfile}
              cafeTimezone={cafeTimezone}
              expiryDate={balance.expiryDate ?? balanceRecord?.expiryDate}
            />
          </Grid>
        )}
      </Grid>
    );
  }, [
    session,
    balance,
    device,
    startTime,
    isActive,
    balanceRecord?.deductionProfile,
    balanceRecord?.expiryDate,
    cafeTimezone,
  ]);

  const sections: DetailPageSection[] = (() => {
    if (!session) return [];

    const result: DetailPageSection[] = [
      {
        title: 'Session timeline',
        content: (
          <TimelineCard>
            <Grid container spacing={2}>
              <DetailField label="Session ID">
                <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                  {id}
                </Typography>
              </DetailField>
              <DetailField label="Start time">
                <Typography variant="body1">
                  {startTime ? formatDate(startTime, 'datetime') : 'N/A'}
                </Typography>
              </DetailField>
              <DetailField label="End time">
                <Typography variant="body1">
                  {endTime ? formatDate(endTime, 'datetime') : 'Still active'}
                </Typography>
              </DetailField>
              <DetailField label="Duration">
                <Typography variant="body1" fontWeight={600} color="primary.main">
                  {formatDuration(durationMinutes)}
                </Typography>
              </DetailField>
              <DetailField label="Time credits consumed">
                <Typography variant="body1">
                  {timeCreditsConsumed ? `${timeCreditsConsumed} minutes` : 'N/A'}
                </Typography>
              </DetailField>
            </Grid>
          </TimelineCard>
        ),
      },
    ];

    if (balance) {
      result.push({
        title: 'Player & plan',
        content: (
          <TimelineCard>
            <Grid container spacing={2}>
              <DetailField label="Player username">
                <Typography variant="body1">{balance.player?.username || 'N/A'}</Typography>
              </DetailField>
              <DetailField label="Player name">
                <Typography variant="body1">
                  {balance.player?.firstName && balance.player?.lastName
                    ? `${balance.player.firstName} ${balance.player.lastName}`
                    : 'N/A'}
                </Typography>
              </DetailField>
              <DetailField label="Plan name">
                <Typography variant="body1">{balance.plan?.name || 'N/A'}</Typography>
              </DetailField>
              <DetailField label={isActive ? 'Time remaining' : 'Time at end'}>
                {isActive && balance.remainingMinutes != null ? (
                  <SessionRemainingClock
                    sessionStartTime={startTime ?? session.startTime}
                    remainingMinutes={balance.remainingMinutes}
                    timeCreditsConsumed={session.timeCreditsConsumed}
                    deductionProfile={balance.deductionProfile ?? balanceRecord?.deductionProfile}
                    cafeTimezone={cafeTimezone}
                    expiryDate={balance.expiryDate ?? balanceRecord?.expiryDate}
                  />
                ) : (
                  <Typography variant="body1" color="text.secondary">
                    Expired
                    {balance.remainingMinutes != null && balance.remainingMinutes > 0
                      ? ` · ${formatRemainingLabel(balance.remainingMinutes)} wallet left`
                      : ''}
                  </Typography>
                )}
              </DetailField>
              <DetailField label="Plan kind">
                <Chip
                  label={balance.kind === 'happy_hours' ? 'Happy Hours' : 'Time Plan'}
                  size="small"
                  color={balance.kind === 'happy_hours' ? 'secondary' : 'primary'}
                  sx={{ mt: 0.5 }}
                />
              </DetailField>
              <DetailField label="Balance status">
                <Typography variant="body1">{balance.status || 'N/A'}</Typography>
              </DetailField>
            </Grid>
          </TimelineCard>
        ),
      });
    }

    if (device) {
      result.push({
        title: 'Device',
        content: (
          <TimelineCard>
            <Grid container spacing={2}>
              <DetailField label="Device name">
                <Typography variant="body1">{device.name || 'N/A'}</Typography>
              </DetailField>
              <DetailField label="Device type">
                <Typography variant="body1">{device.deviceType || 'N/A'}</Typography>
              </DetailField>
              <DetailField label="Serial number">
                <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                  {(device as { serialNumber?: string }).serialNumber ?? 'N/A'}
                </Typography>
              </DetailField>
              <DetailField label="Status">
                <Typography variant="body1">{device.status || 'N/A'}</Typography>
              </DetailField>
            </Grid>
          </TimelineCard>
        ),
      });
    }

    result.push({
      title: 'Metadata',
      content: (
        <TimelineCard>
          <Grid container spacing={2}>
            <DetailField label="Created at">
              <Typography variant="body1">
                {createdAt ? formatDate(createdAt, 'datetime') : 'N/A'}
              </Typography>
            </DetailField>
            <DetailField label="Last updated">
              <Typography variant="body1">
                {updatedAt ? formatDate(updatedAt, 'datetime') : 'N/A'}
              </Typography>
            </DetailField>
          </Grid>
        </TimelineCard>
      ),
    });

    if (isActive) {
      result.push({
        title: 'End session',
        content: (
          <Box id="end-session">
            <TimelineCard>
              <Box
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: 1,
                  mb: 2,
                }}
              >
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                  <Stop color="error" />
                  <Typography variant="subtitle1" fontWeight={600}>
                    End session
                  </Typography>
                </Box>
                <Button
                  variant="contained"
                  color="error"
                  size="small"
                  disabled={isForcing}
                  onClick={() => setConfirmForce(true)}
                  sx={{ display: { xs: 'none', md: 'inline-flex' } }}
                >
                  Force-end
                </Button>
              </Box>
              <Divider sx={{ mb: 3 }} />
              <FormBuilder<EndSessionFormData>
                fields={endSessionFormFields}
                schema={endSessionSchema}
                defaultValues={{
                  endTime: undefined,
                }}
                mode="edit"
                onSubmit={handleEndSession}
                onCancel={handleCancel}
                loading={isSubmitting}
                showCancel
                submitLabel="End Session"
                cancelLabel="Back to List"
                buttonAlign="right"
                spacing={3}
              />
            </TimelineCard>
          </Box>
        ),
      });
    }

    return result;
  })();

  const banner =
    error || success ? (
      <Box sx={{ mb: 2 }}>
        {error && (
          <Alert severity="error" sx={{ mb: success ? 1 : 0 }} onClose={() => setError(undefined)}>
            {error}
          </Alert>
        )}
        {success && <Alert severity="success">{success}</Alert>}
      </Box>
    ) : undefined;

  const sessionActions = isActive ? (
    <Stack direction="row" spacing={2} flexWrap="wrap" useFlexGap>
      <Button variant="contained" startIcon={<Timer />} onClick={handleBuyMoreTime}>
        Buy more time
      </Button>
      <Button variant="outlined" onClick={scrollToEndSession}>
        End session
      </Button>
      <Button variant="text" color="error" onClick={() => setConfirmForce(true)}>
        Force-end
      </Button>
    </Stack>
  ) : (
    <Button variant="outlined" onClick={handleCancel}>
      Back to List
    </Button>
  );

  return (
    <>
      <Box sx={{ pb: { xs: isActive ? 12 : 0, md: 0 } }}>
        <DetailPage
          title="Session details"
          description="View session information and manage session status"
          backTo="/sessions"
          backLabel="Back to sessions"
          breadcrumbs={[{ label: 'Sessions', to: '/sessions' }, { label: 'Session details' }]}
          isLoading={isLoading}
          error={
            !isLoading && (fetchError || !session)
              ? fetchError instanceof Error
                ? fetchError.message
                : 'Session not found'
              : null
          }
          onRetry={() => void refetch()}
          status={{
            label: isActive ? 'Active' : 'Completed',
            color: isActive ? 'success' : 'default',
          }}
          banner={banner}
          summary={summary}
          actions={sessionActions}
          sections={sections}
        />
      </Box>

      {isActive && (
        <SessionDetailActionBar
          onBuyMoreTime={handleBuyMoreTime}
          onEndSession={scrollToEndSession}
          onForceEnd={() => setConfirmForce(true)}
        />
      )}

      <Dialog open={confirmForce} onClose={() => setConfirmForce(false)}>
        <DialogTitle>Force-end this session?</DialogTitle>
        <DialogContent>
          <DialogContentText>{forceEndDialogMessage(device?.deviceType)}</DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmForce(false)} disabled={isForcing}>
            Cancel
          </Button>
          <Button
            color="error"
            variant="contained"
            onClick={handleForceEndConfirm}
            disabled={isForcing}
          >
            {isForcing ? 'Ending…' : 'Force-end'}
          </Button>
        </DialogActions>
      </Dialog>

      <StaffTotpDialog
        open={totpDialog.open}
        title={totpDialog.mode === 'force' ? 'Force-end session' : 'End session'}
        description="Enter your authenticator code to confirm."
        confirmLabel={totpDialog.mode === 'force' ? 'Force-end' : 'End session'}
        confirmColor={totpDialog.mode === 'force' ? 'error' : 'primary'}
        loading={totpDialog.loading || isSubmitting || isForcing}
        onClose={() => {
          setTotpDialog({ open: false, mode: 'end', loading: false });
          setPendingEndData(null);
        }}
        onConfirm={handleTotpConfirm}
      />
    </>
  );
}
