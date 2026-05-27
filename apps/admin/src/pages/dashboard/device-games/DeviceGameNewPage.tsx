import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Alert, Box, Paper, Typography } from '@mui/material';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type AssignGameToDeviceFormData,
  assignGameToDeviceDefaultValues,
  assignGameToDeviceSchema,
} from '../../../../src/containers/device-games/schemas/device-game-schema';
import { assignGameToDevice } from '../../../services/device-games/add';
import { getDevices } from '../../../services/devices/list';
import { getGames } from '../../../services/games/list';

const assignGameToDeviceFormFields: FieldConfig<AssignGameToDeviceFormData>[] = [
  {
    name: 'deviceId',
    label: 'Device',
    type: 'search',
    onSearch: async (query: string) => {
      const data = await getDevices({ limit: 100, name: query });
      return data.data.map((device) => ({
        label: device.name,
        id: device.id,
      }));
    },
  },
  {
    name: 'gameId',
    label: 'Game',
    type: 'search',
    onSearch: async (query: string) => {
      const data = await getGames({ limit: 100, title: query });
      return data.data.map((game) => ({
        label: game.title,
        id: game.id,
      }));
    },
  },
  {
    name: 'installationDate',
    label: 'Installation Date',
    type: 'date',
    placeholder: 'Select installation date',
    required: true,
    gridCols: 6,
    helperText: 'Select the date the game was installed on the device',
  },
  {
    name: 'isActive',
    label: 'Active',
    type: 'switch',
    gridCols: 6,
    helperText: 'Toggle if the game is active',
  },
];

export default function AssignGameToDevicePage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const onSubmit = async (data: AssignGameToDeviceFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    try {
      await assignGameToDevice({
        deviceId: data.deviceId,
        gameId: data.gameId,
        installationDate: data.installationDate
          ? new Date(data.installationDate).toISOString()
          : undefined,
        isActive: data.isActive,
      });

      setSuccess('Game assigned to device successfully!');

      // Navigate back to device-games list after a short delay
      setTimeout(() => {
        navigate('/device-games');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to assign game to device');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/device-games');
  };

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
            Assign Game to Device
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Select a device and game to create an assignment
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

        <FormBuilder<AssignGameToDeviceFormData>
          fields={assignGameToDeviceFormFields}
          schema={assignGameToDeviceSchema}
          defaultValues={assignGameToDeviceDefaultValues}
          mode="add"
          onSubmit={onSubmit}
          onCancel={handleCancel}
          loading={loading}
          error={error}
          success={success}
          showCancel
          showReset
          submitLabel="Assign Game to Device"
          cancelLabel="Cancel"
          resetLabel="Reset Form"
          buttonAlign="right"
          spacing={3}
        />
      </Paper>
    </LocalizationProvider>
  );
}
