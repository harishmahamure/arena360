import { type FieldConfig, FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { formatDate } from '@gaming-cafe/utils';
import { Stop } from '@mui/icons-material';
import {
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Divider,
  GridLegacy as Grid,
  Paper,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type EndSessionFormData,
  endSessionSchema,
} from '../../../containers/sessions/schemas/session-schema';
import { getDeviceById } from '../../../services/devices/getById';
import { getPlanById } from '../../../services/plans/getById';
import { getPlayerPlanById } from '../../../services/player-plans/getById';
import { getPlayerById } from '../../../services/players/getById';
import { getSessionById } from '../../../services/sessions/getById';
import { endSession } from '../../../services/sessions/update';

const formatDuration = (minutes?: number | null) => {
  if (!minutes) return 'N/A';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
};

export default function ViewSessionPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const {
    data: session,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['session', id],
    queryFn: () => getSessionById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
  });

  const { data: playerPlanRecord } = useQuery({
    queryKey: ['player-plan', session?.playerPlanId],
    queryFn: () => {
      const playerPlanId = session?.playerPlanId;
      if (!playerPlanId) {
        throw new Error('Missing player plan ID');
      }
      return getPlayerPlanById(playerPlanId);
    },
    enabled: !!session?.playerPlanId && !session?.playerPlan,
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

  const playerId = session?.playerPlan?.playerId ?? playerPlanRecord?.playerId;
  const planId = session?.playerPlan?.planId ?? playerPlanRecord?.planId;

  const { data: playerRecord } = useQuery({
    queryKey: ['player', playerId],
    queryFn: () => {
      if (!playerId) {
        throw new Error('Missing player ID');
      }
      return getPlayerById(playerId);
    },
    enabled: !!playerId && !session?.playerPlan?.player,
  });

  const { data: planRecord } = useQuery({
    queryKey: ['plan', planId],
    queryFn: () => {
      if (!planId) {
        throw new Error('Missing plan ID');
      }
      return getPlanById(planId);
    },
    enabled: !!planId && !session?.playerPlan?.plan,
  });

  const startTime = session?.startTime;
  const endTime = session?.endTime;
  const durationMinutes = session?.durationMinutes;
  const timeCreditsConsumed = session?.timeCreditsConsumed;
  const createdAt = session?.createdAt;
  const updatedAt = session?.updatedAt;
  const playerPlan =
    session?.playerPlan ??
    (playerPlanRecord
      ? {
          ...playerPlanRecord,
          player: playerRecord,
          plan: planRecord,
        }
      : undefined);
  const device = session?.device ?? deviceRecord;

  const isActive = !endTime;

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

  const handleEndSession = async (data: EndSessionFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    try {
      await endSession(id as string, {
        endTime: data.endTime || undefined,
      });

      setSuccess('Session ended successfully!');

      // Refetch session data
      await refetch();

      // Navigate back to sessions list after a short delay
      setTimeout(() => {
        navigate('/sessions');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to end session');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/sessions');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <Paper elevation={0} sx={{ p: 4 }}>
        <Box
          sx={{
            mb: 4,
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}
        >
          <Box>
            <Typography variant="h4" fontWeight={600} gutterBottom>
              Session Details
            </Typography>
            <Typography variant="body2" color="text.secondary">
              View session information and manage session status
            </Typography>
          </Box>
          <Chip
            label={isActive ? 'Active' : 'Completed'}
            color={isActive ? 'success' : 'default'}
            size="medium"
          />
        </Box>

        {/* Session Information */}
        <Card variant="outlined" sx={{ mb: 4 }}>
          <CardContent>
            <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
              Session Information
            </Typography>
            <Divider sx={{ mb: 2 }} />
            <Grid container spacing={2}>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Session ID
                </Typography>
                <Typography variant="body1" sx={{ fontFamily: 'monospace', fontSize: '0.875rem' }}>
                  {id}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Start Time
                </Typography>
                <Typography variant="body1">
                  {startTime ? formatDate(startTime, 'datetime') : 'N/A'}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  End Time
                </Typography>
                <Typography variant="body1">
                  {endTime ? formatDate(endTime, 'datetime') : 'Still Active'}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Duration
                </Typography>
                <Typography variant="h6" color="primary">
                  {formatDuration(durationMinutes)}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Time Credits Consumed
                </Typography>
                <Typography variant="body1">
                  {timeCreditsConsumed ? `${timeCreditsConsumed} minutes` : 'N/A'}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Status
                </Typography>
                <Chip
                  label={isActive ? 'Active' : 'Completed'}
                  color={isActive ? 'success' : 'default'}
                  size="small"
                  sx={{ mt: 0.5 }}
                />
              </Grid>
            </Grid>
          </CardContent>
        </Card>

        {/* Player Information */}
        {playerPlan && (
          <Card variant="outlined" sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
                Player & Plan Information
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Player Username
                  </Typography>
                  <Typography variant="body1">{playerPlan.player?.username || 'N/A'}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Player Name
                  </Typography>
                  <Typography variant="body1">
                    {playerPlan.player?.firstName && playerPlan.player?.lastName
                      ? `${playerPlan.player.firstName} ${playerPlan.player.lastName}`
                      : 'N/A'}
                  </Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Plan Name
                  </Typography>
                  <Typography variant="body1">{playerPlan.plan?.name || 'N/A'}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Remaining Time Credits
                  </Typography>
                  <Typography variant="body1">
                    {playerPlan.remainingTimeCredits
                      ? `${playerPlan.remainingTimeCredits} minutes`
                      : 'N/A'}
                  </Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Plan Status
                  </Typography>
                  <Typography variant="body1">{playerPlan.status || 'N/A'}</Typography>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        )}

        {/* Device Information */}
        {device && (
          <Card variant="outlined" sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
                Device Information
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Device Name
                  </Typography>
                  <Typography variant="body1">{device.name || 'N/A'}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Device Type
                  </Typography>
                  <Typography variant="body1">{device.deviceType || 'N/A'}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Serial Number
                  </Typography>
                  <Typography
                    variant="body1"
                    sx={{ fontFamily: 'monospace', fontSize: '0.875rem' }}
                  >
                    {(device as { serialNumber?: string }).serialNumber ?? 'N/A'}
                  </Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Status
                  </Typography>
                  <Typography variant="body1">{device.status || 'N/A'}</Typography>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        )}

        {/* Metadata */}
        <Card variant="outlined" sx={{ mb: 4 }}>
          <CardContent>
            <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
              Metadata
            </Typography>
            <Divider sx={{ mb: 2 }} />
            <Grid container spacing={2}>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Created At
                </Typography>
                <Typography variant="body1">
                  {createdAt ? formatDate(createdAt, 'datetime') : 'N/A'}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Last Updated
                </Typography>
                <Typography variant="body1">
                  {updatedAt ? formatDate(updatedAt, 'datetime') : 'N/A'}
                </Typography>
              </Grid>
            </Grid>
          </CardContent>
        </Card>

        {/* End Session Form - Only show for active sessions */}
        {isActive && (
          <Card variant="outlined">
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
                <Stop color="error" />
                <Typography variant="h6">End Session</Typography>
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
                error={error}
                success={success}
                showCancel
                submitLabel="End Session"
                cancelLabel="Back to List"
                buttonAlign="right"
                spacing={3}
              />
            </CardContent>
          </Card>
        )}

        {/* Back button for completed sessions */}
        {!isActive && (
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 3 }}>
            <Button variant="outlined" onClick={handleCancel}>
              Back to List
            </Button>
          </Box>
        )}
      </Paper>
    </Box>
  );
}
