import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Build, CheckCircle, Error as ErrorIcon, Schedule } from '@mui/icons-material';
import { Box, Chip, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { ConsoleTvProvisioningCard } from '../../../components/ConsoleTvProvisioningCard';
import { KioskFingerprintCard } from '../../../components/KioskFingerprintCard';
import { KioskProvisioningCard } from '../../../components/KioskProvisioningCard';
import {
  type CreateDeviceFormData,
  createDeviceSchema,
  deviceStatusOptions,
  deviceSubTypeOptions,
  deviceTypeOptions,
} from '../../../containers/devices/schemas/device-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getDeviceById } from '../../../services/devices/getById';
import { DeviceStatus } from '../../../services/devices/list';
import { updateDevice } from '../../../services/devices/update';

const editDeviceFormFields: FieldConfig<CreateDeviceFormData>[] = [
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
    helperText: 'Current operational status',
  },
];

const getStatusColor = (status: DeviceStatusValue) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return 'success';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'warning';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'error';
    case DeviceStatus.IN_USE:
      return 'info';
    case DeviceStatus.AVAILABLE:
      return 'success';
    default:
      return 'default';
  }
};

const getStatusIcon = (status: DeviceStatusValue) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return <CheckCircle fontSize="small" />;
    case DeviceStatus.UNDER_MAINTENANCE:
      return <Build fontSize="small" />;
    case DeviceStatus.OUT_OF_SERVICE:
      return <ErrorIcon fontSize="small" />;
    case DeviceStatus.IN_USE:
      return <Schedule fontSize="small" />;
    case DeviceStatus.AVAILABLE:
      return <CheckCircle fontSize="small" />;
    default:
      return null;
  }
};

const getStatusLabel = (status: DeviceStatusValue) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return 'Operational';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'Under Maintenance';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'Out of Order';
    case DeviceStatus.IN_USE:
      return 'In Use';
    case DeviceStatus.AVAILABLE:
      return 'Available';
    default:
      return status;
  }
};

export default function EditDevicePage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const { can } = usePermissions();
  const canWrite = can(Permission.DevicesWrite);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: device, isLoading } = useQuery({
    queryKey: ['device', id],
    queryFn: () => getDeviceById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
  });

  const handleSubmit = async (data: CreateDeviceFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.name || !data.deviceType || !data.deviceSubType) {
      setError('Device name, type, and sub type are required');
      setIsSubmitting(false);
      return;
    }

    try {
      await updateDevice(id as string, {
        name: data.name,
        deviceType: data.deviceType,
        deviceSubType: data.deviceSubType,
        serialNumber: data.serialNumber || undefined,
        localIpAddress: data.localIpAddress || undefined,
        location: data.location || undefined,
        status: data.status as DeviceStatusValue,
      });

      setSuccess('Device updated successfully!');

      setTimeout(() => {
        navigate('/devices');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update device');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/devices');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  const deviceData = device;

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box
        sx={{
          mb: 4,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'flex-start',
        }}
      >
        <Box>
          <Typography variant="h4" fontWeight={600} gutterBottom>
            Edit Device
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Update details for {deviceData?.name}
          </Typography>
        </Box>
        {deviceData?.status && (
          <Chip
            icon={getStatusIcon(deviceData.status) as React.ReactElement}
            label={getStatusLabel(deviceData.status)}
            color={getStatusColor(deviceData.status)}
            sx={{ fontWeight: 500 }}
          />
        )}
      </Box>

      {deviceData &&
        (deviceData.deviceType === 'PS5' || deviceData.deviceType === 'PS4' ? (
          <ConsoleTvProvisioningCard
            deviceName={deviceData.name}
            registrationStatus={deviceData.registrationStatus}
          />
        ) : (
          <KioskProvisioningCard
            deviceName={deviceData.name}
            registrationStatus={deviceData.registrationStatus}
          />
        ))}

      {deviceData && (
        <KioskFingerprintCard
          registeredKiosk={deviceData.registeredKiosk}
          registrationStatus={deviceData.registrationStatus}
        />
      )}

      <FormBuilder<CreateDeviceFormData>
        fields={editDeviceFormFields}
        schema={createDeviceSchema}
        defaultValues={{
          name: deviceData?.name || '',
          deviceType: (deviceData?.deviceType || '') as CreateDeviceFormData['deviceType'],
          deviceSubType: (deviceData?.deviceSubType || '') as CreateDeviceFormData['deviceSubType'],
          serialNumber: deviceData?.serialNumber || '',
          localIpAddress: deviceData?.localIpAddress || '',
          location: deviceData?.location || '',
          status: deviceData?.status || DeviceStatus.OPERATIONAL,
        }}
        mode={canWrite ? 'edit' : 'view'}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel={canWrite}
        showReset={canWrite}
        submitLabel="Update Device"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
