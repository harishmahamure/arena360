import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItem,
  ListItemText,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
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

const NEXT_STEPS = [
  'Go to the kiosk PC and open the Arena360 kiosk app.',
  'Sign in with your admin credentials and OTP on the device.',
  'Name the station to match this device record — provisioning completes automatically.',
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

  const handleDialogClose = () => {
    if (createdDevice?.id) {
      navigate(`/devices/${createdDevice.id}`);
    } else {
      navigate('/devices');
    }
    setCreatedDevice(null);
  };

  return (
    <FormPage
      title="Add New Device"
      description="Register a new gaming device or station in your game zone"
      backTo="/devices"
      backLabel="Back to devices"
      breadcrumbs={[{ label: 'Devices', to: '/devices' }, { label: 'New device' }]}
    >
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
        <DialogTitle>Device created — provision on kiosk</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            <strong>{createdDevice?.name}</strong> was added. Complete provisioning on the kiosk PC:
          </Typography>
          <List dense>
            {NEXT_STEPS.map((step) => (
              <ListItem key={step} disableGutters sx={{ py: 0.25 }}>
                <ListItemText primary={step} primaryTypographyProps={{ variant: 'body2' }} />
              </ListItem>
            ))}
          </List>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button variant="contained" onClick={handleDialogClose}>
            View device
          </Button>
        </DialogActions>
      </Dialog>
    </FormPage>
  );
}
