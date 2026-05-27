import { yupResolver } from '@hookform/resolvers/yup';
import { Devices, SportsEsports } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  FormControlLabel,
  GridLegacy as Grid,
  Paper,
  Skeleton,
  Switch,
  Typography,
} from '@mui/material';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { useNavigate, useParams } from 'react-router-dom';
import * as yup from 'yup';
import { getDeviceGameById } from '../../../services/device-games/getById';
import { updateDeviceGame } from '../../../services/device-games/update';

// Schema for editing (only editable fields)
const editDeviceGameSchema = yup.object({
  installationDate: yup
    .date()
    .optional()
    .nullable()
    .transform((value, originalValue) => {
      if (originalValue === '' || originalValue === null) return null;
      return value;
    }),
  isActive: yup.boolean().optional().default(true),
});

type EditDeviceGameFormData = yup.InferType<typeof editDeviceGameSchema>;

export default function EditDeviceGamePage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  // Fetch the device-game assignment
  const { data: deviceGame, isLoading: loadingDeviceGame } = useQuery({
    queryKey: ['device-game', id],
    queryFn: () => getDeviceGameById(id as string),
    enabled: !!id,
  });

  const { control, handleSubmit } = useForm({
    resolver: yupResolver(editDeviceGameSchema),
    values: {
      installationDate: deviceGame?.installationDate
        ? new Date(deviceGame.installationDate)
        : null,
      isActive: deviceGame?.isActive ?? true,
    },
  });

  const onSubmit = async (data: EditDeviceGameFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    try {
      await updateDeviceGame(id as string, {
        installationDate: data.installationDate
          ? new Date(data.installationDate).toISOString()
          : undefined,
        isActive: data.isActive,
      });

      setSuccess('Assignment updated successfully!');

      // Navigate back to device-games list after a short delay
      setTimeout(() => {
        navigate('/device-games');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update assignment');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/device-games');
  };

  if (loadingDeviceGame) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <Skeleton variant="text" width={300} height={40} sx={{ mb: 2 }} />
        <Skeleton variant="text" width={200} height={24} sx={{ mb: 4 }} />
        <Grid container spacing={3}>
          <Grid item xs={12} md={6}>
            <Skeleton variant="rounded" height={100} />
          </Grid>
          <Grid item xs={12} md={6}>
            <Skeleton variant="rounded" height={100} />
          </Grid>
          <Grid item xs={12} md={6}>
            <Skeleton variant="rounded" height={56} />
          </Grid>
          <Grid item xs={12} md={6}>
            <Skeleton variant="rounded" height={56} />
          </Grid>
        </Grid>
      </Paper>
    );
  }

  return (
    <LocalizationProvider dateAdapter={AdapterDateFns}>
      <Paper
        elevation={0}
        sx={{
          p: 4,
        }}
      >
        <Box sx={{ mb: 4 }}>
          <Typography variant="h4" fontWeight={600} gutterBottom>
            Edit Device-Game Assignment
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Update the assignment details
          </Typography>
        </Box>

        {error && (
          <Alert severity="error" sx={{ mb: 3 }}>
            {error}
          </Alert>
        )}

        {success && (
          <Alert severity="success" sx={{ mb: 3 }}>
            {success}
          </Alert>
        )}

        {/* Display Device and Game Info (read-only) */}
        <Grid container spacing={3} sx={{ mb: 4 }}>
          <Grid item xs={12} md={6}>
            <Card variant="outlined">
              <CardContent>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                  <Box
                    sx={{
                      width: 48,
                      height: 48,
                      borderRadius: 1,
                      bgcolor: 'primary.main',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      color: 'white',
                    }}
                  >
                    <Devices />
                  </Box>
                  <Box>
                    <Typography variant="overline" color="text.secondary">
                      Device
                    </Typography>
                    <Typography variant="h6" fontWeight={500}>
                      {deviceGame?.device?.name || 'Unknown Device'}
                    </Typography>
                    <Typography variant="body2" color="text.secondary">
                      {deviceGame?.device?.deviceType || '-'}
                      {deviceGame?.device?.location &&
                        ` • ${deviceGame.device.location}`}
                    </Typography>
                  </Box>
                </Box>
              </CardContent>
            </Card>
          </Grid>

          <Grid item xs={12} md={6}>
            <Card variant="outlined">
              <CardContent>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                  <Box
                    sx={{
                      width: 48,
                      height: 48,
                      borderRadius: 1,
                      bgcolor: 'secondary.main',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      color: 'white',
                    }}
                  >
                    <SportsEsports />
                  </Box>
                  <Box>
                    <Typography variant="overline" color="text.secondary">
                      Game
                    </Typography>
                    <Typography variant="h6" fontWeight={500}>
                      {deviceGame?.game?.title || 'Unknown Game'}
                    </Typography>
                    <Typography variant="body2" color="text.secondary">
                      {deviceGame?.game?.genre || 'No genre'}
                    </Typography>
                  </Box>
                </Box>
              </CardContent>
            </Card>
          </Grid>
        </Grid>

        <Typography variant="subtitle1" fontWeight={500} sx={{ mb: 2 }}>
          Edit Assignment Details
        </Typography>

        <form onSubmit={handleSubmit(onSubmit)}>
          <Grid container spacing={3}>
            {/* Installation Date */}
            <Grid item xs={12} md={6}>
              <Controller
                name="installationDate"
                control={control}
                render={({ field }) => (
                  <DatePicker
                    label="Installation Date"
                    value={field.value}
                    onChange={(newValue) => field.onChange(newValue)}
                    slotProps={{
                      textField: {
                        fullWidth: true,
                        helperText: 'When the game was/will be installed (optional)',
                      },
                    }}
                  />
                )}
              />
            </Grid>

            {/* Is Active Switch */}
            <Grid item xs={12} md={6}>
              <Box
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  height: '100%',
                  pt: 1,
                }}
              >
                <Controller
                  name="isActive"
                  control={control}
                  render={({ field }) => (
                    <FormControlLabel
                      control={
                        <Switch
                          checked={field.value}
                          onChange={(e) => field.onChange(e.target.checked)}
                          color="primary"
                        />
                      }
                      label="Active (Game available on device)"
                    />
                  )}
                />
              </Box>
            </Grid>

            {/* Action Buttons */}
            <Grid item xs={12}>
              <Box
                sx={{
                  display: 'flex',
                  gap: 2,
                  justifyContent: 'flex-end',
                  mt: 2,
                }}
              >
                <Button variant="outlined" onClick={handleCancel} disabled={loading}>
                  Cancel
                </Button>
                <Button
                  type="submit"
                  variant="contained"
                  disabled={loading}
                  startIcon={loading && <CircularProgress size={20} />}
                >
                  {loading ? 'Saving...' : 'Save Changes'}
                </Button>
              </Box>
            </Grid>
          </Grid>
        </form>
      </Paper>
    </LocalizationProvider>
  );
}
