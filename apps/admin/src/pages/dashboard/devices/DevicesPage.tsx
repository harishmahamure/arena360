import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { Computer, Edit, SportsEsports, Tv } from '@mui/icons-material';
import { Box, Chip, debounce, MenuItem, TextField, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getVenueLocations } from '../../../services/config';
import { type DeviceResponse, getDevices } from '../../../services/devices/list';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDate } from '../../../utils/date';
import {
  deviceStatusColor,
  deviceStatusIcon,
  deviceStatusLabel,
} from '../../../utils/deviceStatusDisplay';
import { currentOrganizationId } from '../access/LocationsPanel';

const getStatusColor = deviceStatusColor;
const getStatusIcon = deviceStatusIcon;
const getStatusLabel = deviceStatusLabel;

const getDeviceIcon = (deviceType: string) => {
  const type = deviceType.toUpperCase();
  if (type === 'PC') {
    return <Computer />;
  }
  if (type === 'PS5' || type === 'PS4' || type === 'CONSOLE') {
    return <SportsEsports />;
  }
  return <Tv />;
};

export default function DevicesPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const statusFilter = searchParams.get('status') as DeviceStatusValue | null;
  const typeFilter = searchParams.get('type');
  const locationId = searchParams.get('locationId') ?? '';
  const organizationId = currentOrganizationId();
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
  });

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

  const {
    data,
    isLoading,
    error: listError,
    refetch,
  } = useQuery({
    queryKey: ['devices', debouncedSearch, page, statusFilter, typeFilter, locationId],
    queryFn: () =>
      getDevices({
        name: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        ...(statusFilter && { status: statusFilter }),
        ...(typeFilter && { deviceType: typeFilter }),
        ...(locationId && { locationId }),
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
      format: (_value, row) =>
        locations.data?.find((location) => location.id === row?.locationId)?.name ??
        row?.location ??
        '—',
    },
    {
      id: 'registrationStatus',
      label: 'Registration',
      minWidth: 110,
      align: 'center',
      format: (value) => {
        const status = value as string | undefined;
        if (status === 'registered') {
          return <Chip label="Registered" color="success" size="small" />;
        }
        if (status === 'unregistered') {
          return <Chip label="Pending" color="warning" size="small" />;
        }
        return '—';
      },
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 140,
      align: 'center',
      format: (value) => (
        <Chip
          icon={getStatusIcon(value as DeviceStatusValue) ?? undefined}
          label={getStatusLabel(value as DeviceStatusValue)}
          color={getStatusColor(value as DeviceStatusValue)}
          size="small"
        />
      ),
    },
    {
      id: 'createdAt',
      label: 'Added',
      minWidth: 110,
      format: (value) => formatDisplayDate(value as string),
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
    <>
      <TextField
        select
        size="small"
        label="Venue location"
        value={locationId}
        sx={{ minWidth: 220, mb: 2 }}
        onChange={(event) => {
          const next = new URLSearchParams(searchParams);
          if (event.target.value) next.set('locationId', event.target.value);
          else next.delete('locationId');
          next.delete('page');
          navigate(`/devices?${next.toString()}`);
        }}
      >
        <MenuItem value="">All permitted locations</MenuItem>
        {locations.data?.map((location) => (
          <MenuItem key={location.id} value={location.id}>
            {location.name}
          </MenuItem>
        ))}
      </TextField>
      <ListPage<DeviceResponse>
        error={listError ? 'Could not load records. Please try again.' : null}
        onRetry={() => void refetch()}
        title="Devices"
        description="Manage your game zone devices and stations here."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        showSearch
        searchValue={inputValue}
        onSearchChange={handleSearch}
        onSearchClear={handleClearSearch}
        onAddClick={canWrite ? handleAddNewDevice : undefined}
        addButtonLabel="Add Device"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(
              buildListUrl('/devices', value, {
                status: statusFilter ?? undefined,
                type: typeFilter ?? undefined,
                locationId: locationId || undefined,
              }),
            ),
        }}
      />
    </>
  );
}
