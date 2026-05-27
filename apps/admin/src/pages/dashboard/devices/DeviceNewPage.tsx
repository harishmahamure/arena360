import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateDeviceFormData,
  createDeviceDefaultValues,
  createDeviceSchema,
  deviceStatusOptions,
  deviceTypeOptions,
} from '../../../../src/containers/devices/schemas/device-schema';
import { addDevice } from '../../../services/devices/add';
import { DeviceStatus } from '../../../services/devices/list';

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
    helperText: 'Type of gaming device',
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
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreateDeviceFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.name || !data.deviceType) {
      setError('Device name and type are required');
      setLoading(false);
      return;
    }

    try {
      await addDevice({
        name: data.name,
        deviceType: data.deviceType,
        serialNumber: data.serialNumber || undefined,
        localIpAddress: data.localIpAddress || undefined,
        location: data.location || undefined,
        status: (data.status as DeviceStatus) || DeviceStatus.OPERATIONAL,
      });

      setSuccess('Device created successfully!');

      setTimeout(() => {
        navigate('/devices');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create device');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/devices');
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
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
        success={success}
        showCancel
        showReset
        submitLabel="Create Device"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
