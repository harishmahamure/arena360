import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import {
  Build,
  CheckCircle,
  Computer,
  Edit,
  Error as ErrorIcon,
  PlayArrow,
  SportsEsports,
  Tv,
} from '@mui/icons-material';
import { Box, Chip, debounce, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { type DeviceResponse, DeviceStatus, getDevices } from '../../../services/devices/list';

const getStatusColor = (status: DeviceStatus) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return 'success';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'warning';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'error';
    case DeviceStatus.IN_USE:
      return 'primary';
    case DeviceStatus.AVAILABLE:
      return 'success';
    default:
      return 'default';
  }
};

const getStatusIcon = (status: DeviceStatus) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return <CheckCircle fontSize="small" />;
    case DeviceStatus.UNDER_MAINTENANCE:
      return <Build fontSize="small" />;
    case DeviceStatus.OUT_OF_SERVICE:
      return <ErrorIcon fontSize="small" />;
    case DeviceStatus.IN_USE:
      return <PlayArrow fontSize="small" />;
    case DeviceStatus.AVAILABLE:
      return <CheckCircle fontSize="small" />;
    default:
      return null;
  }
};

const getStatusLabel = (status: DeviceStatus) => {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return 'Operational';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'Maintenance';
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

const getDeviceIcon = (deviceType: string) => {
  const type = deviceType.toLowerCase();
  if (type.includes('playstation') || type.includes('xbox') || type.includes('nintendo')) {
    return <SportsEsports />;
  }
  if (type.includes('pc') || type.includes('computer')) {
    return <Computer />;
  }
  return <Tv />;
};

export default function DevicesPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const statusFilter = searchParams.get('status') as DeviceStatus | null;
  const typeFilter = searchParams.get('type');

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.DevicesWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewDevice = useCallback(() => {
    navigate('/devices/new');
  }, [navigate]);

  const handleEditDevice = useCallback(
    (id: string) => {
      navigate(`/devices/${id}`);
    },
    [navigate],
  );

  const { data, isLoading } = useQuery({
    queryKey: ['devices', debouncedSearch, page, statusFilter, typeFilter],
    queryFn: () =>
      getDevices({
        name: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        ...(statusFilter && { status: statusFilter }),
        ...(typeFilter && { deviceType: typeFilter }),
      }),
  });

  const handleSearch = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const query = event.target.value;
      setInputValue(query);
      debouncedSetSearch(query);
    },
    [debouncedSetSearch],
  );

  const handleClearSearch = useCallback(() => {
    setInputValue('');
    setDebouncedSearch('');
    debouncedSetSearch.clear();
  }, [debouncedSetSearch]);

  const columns: Column<DeviceResponse>[] = [
    {
      id: 'name',
      label: 'Device',
      minWidth: 200,
      format: (value, row) => (
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <Box
            sx={{
              width: 40,
              height: 40,
              borderRadius: 1,
              bgcolor: 'primary.main',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'white',
            }}
          >
            {getDeviceIcon(row?.deviceType || '')}
          </Box>
          <Box>
            <Typography variant="body2" fontWeight={500}>
              {value as string}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {row?.deviceType}
            </Typography>
          </Box>
        </Box>
      ),
    },
    {
      id: 'serialNumber',
      label: 'Serial Number',
      minWidth: 140,
      format: (value) => (
        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>
          {(value as string) || '—'}
        </Typography>
      ),
    },
    {
      id: 'localIpAddress',
      label: 'IP Address',
      minWidth: 130,
      format: (value) => (
        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>
          {(value as string) || '—'}
        </Typography>
      ),
    },
    {
      id: 'location',
      label: 'Location',
      minWidth: 180,
      format: (value) => (value as string) || '—',
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 140,
      align: 'center',
      format: (value) => (
        <Chip
          icon={getStatusIcon(value as DeviceStatus) ?? undefined}
          label={getStatusLabel(value as DeviceStatus)}
          color={getStatusColor(value as DeviceStatus)}
          size="small"
        />
      ),
    },
    {
      id: 'createdAt',
      label: 'Added',
      minWidth: 110,
      format: (value) =>
        new Date(value as string).toLocaleDateString('en-IN', {
          year: 'numeric',
          month: 'short',
          day: 'numeric',
        }),
    },
  ];

  const actions: Action<DeviceResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Device',
      onClick: (row) => handleEditDevice(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<DeviceResponse>
        title="Devices"
        description="Manage your game zone devices and stations here."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={canWrite ? handleAddNewDevice : undefined}
        addButtonLabel="Add Device"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(value === 1 ? `/devices` : `/devices?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
