import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Paper,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'react-toastify';
import {
  type CreateDeviceFormData,
  createDeviceDefaultValues,
  createDeviceSchema,
  deviceStatusOptions,
  deviceSubTypeOptions,
  deviceTypeOptions,
} from '../../../../src/containers/devices/schemas/device-schema';
import { addDevice } from '../../../services/devices/add';
import { type DeviceResponse, DeviceStatus } from '../../../services/devices/list';

export const deviceFormFields: FieldConfig<CreateDeviceFormData>[] = [
  {
    name: 'name',
    label: 'Device Name',
    type: 'text',
    placeholder: 'e.g., PS5-Station-1',
    required: true,
    gridCols: 6,
    helperText: 'Unique identifier for this device (max 100 characters)',
  },
  {
    name: 'deviceType',
    label: 'Device Type',
    type: 'select',
    placeholder: 'Select device type',
    required: true,
    gridCols: 6,
    options: deviceTypeOptions,
    helperText: 'Must match plan device type for kiosk login',
  },
  {
    name: 'deviceSubType',
    label: 'Device Sub Type',
    type: 'select',
    placeholder: 'Select device sub type',
    required: true,
    gridCols: 6,
    options: deviceSubTypeOptions,
    helperText: 'Must match plan device sub type for kiosk login',
  },
  {
    name: 'serialNumber',
    label: 'Serial Number',
    type: 'text',
    placeholder: 'e.g., SN123456789',
    gridCols: 6,
    helperText: 'Optional serial number (max 100 characters)',
  },
  {
    name: 'localIpAddress',
    label: 'Local IP Address',
    type: 'text',
    placeholder: 'e.g., 192.168.1.100',
    gridCols: 6,
    helperText: 'Optional IP address for network management',
  },
  {
    name: 'location',
    label: 'Location',
    type: 'text',
    placeholder: 'e.g., Main Gaming Floor - Station A',
    gridCols: 12,
    helperText: 'Physical location in the game zone (max 200 characters)',
  },
  {
    name: 'status',
    label: 'Status',
    type: 'select',
    gridCols: 6,
    options: deviceStatusOptions,
    helperText: 'Current operational status (default: Operational)',
  },
];

export default function AddNewDevicePage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [createdDevice, setCreatedDevice] = useState<DeviceResponse | null>(null);

  const handleSubmit = async (data: CreateDeviceFormData) => {
    setLoading(true);
    setError(undefined);

    if (!data.name || !data.deviceType || !data.deviceSubType) {
      setError('Device name, type, and sub type are required');
      setLoading(false);
      return;
    }

    try {
      const device = await addDevice({
        name: data.name,
        deviceType: data.deviceType,
        deviceSubType: data.deviceSubType,
        serialNumber: data.serialNumber || undefined,
        localIpAddress: data.localIpAddress || undefined,
        location: data.location || undefined,
        status: (data.status as DeviceStatusValue) || DeviceStatus.OPERATIONAL,
      });

      setCreatedDevice(device);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create device');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/devices');
  };

  const handleCopyCode = async () => {
    if (!createdDevice?.registrationCode) return;
    try {
      await navigator.clipboard.writeText(createdDevice.registrationCode);
      toast.success('Registration code copied');
    } catch {
      toast.error('Failed to copy code');
    }
  };

  const handleDialogClose = () => {
    if (createdDevice?.id) {
      navigate(`/devices/${createdDevice.id}`);
    } else {
      navigate('/devices');
    }
    setCreatedDevice(null);
  };

  return (
    <Paper elevation={0} sx={{ p: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Device
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Register a new gaming device or station in your game zone
        </Typography>
      </Box>

      <FormBuilder<CreateDeviceFormData>
        fields={deviceFormFields}
        schema={createDeviceSchema}
        defaultValues={createDeviceDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        error={error}
        showCancel
        showReset
        submitLabel="Create Device"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />

      <Dialog open={createdDevice !== null} onClose={handleDialogClose} maxWidth="sm" fullWidth>
        <DialogTitle>Device created — kiosk registration code</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Enter this code on the kiosk at <strong>{createdDevice?.name}</strong>. It expires in 24
            hours.
          </Typography>
          {createdDevice?.registrationCode ? (
            <Box
              sx={{
                p: 2,
                bgcolor: 'action.hover',
                borderRadius: 1,
                textAlign: 'center',
              }}
            >
              <Typography
                variant="h4"
                component="p"
                sx={{ fontFamily: 'monospace', letterSpacing: 4, fontWeight: 700 }}
              >
                {createdDevice.registrationCode}
              </Typography>
              {createdDevice.registrationCodeExpiresAt && (
                <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>
                  Expires {new Date(createdDevice.registrationCodeExpiresAt).toLocaleString()}
                </Typography>
              )}
            </Box>
          ) : (
            <Typography variant="body2" color="warning.main">
              No code was returned. Open the device detail page to generate one.
            </Typography>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          {createdDevice?.registrationCode && (
            <Button startIcon={<ContentCopyIcon />} onClick={() => void handleCopyCode()}>
              Copy code
            </Button>
          )}
          <Button variant="contained" onClick={handleDialogClose}>
            Done
          </Button>
        </DialogActions>
      </Dialog>
    </Paper>
  );
}
