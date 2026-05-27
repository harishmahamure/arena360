import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Cancel, CheckCircle, Delete, Devices, Edit, SportsEsports } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { removeGameFromDevice } from '../../../services/device-games/delete';
import { type DeviceGameResponse, getDeviceGames } from '../../../services/device-games/list';
import { toggleDeviceGameActive } from '../../../services/device-games/update';
import { formatDisplayDate } from '../../../utils/date';

export default function DeviceGamesPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const deviceId = searchParams.get('deviceId') || undefined;
  const gameId = searchParams.get('gameId') || undefined;
  const isActive = searchParams.get('active') === 'false' ? 0 : undefined;

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.DeviceGamesWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAssignGame = useCallback(() => {
    navigate('/device-games/new');
  }, [navigate]);

  const handleEditAssignment = useCallback(
    (id: string) => {
      navigate(`/device-games/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['device-games', debouncedSearch, page, deviceId, gameId, isActive],
    queryFn: () =>
      getDeviceGames({
        page: page,
        deviceId,
        gameId,
        isActive: isActive as 0 | 1 | undefined,
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

  const columns: Column<DeviceGameResponse>[] = [
    {
      id: 'device',
      label: 'Device',
      minWidth: 200,
      format: (_value, row) => (
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
            <Devices />
          </Box>
          <Box>
            <Typography variant="body2" fontWeight={500}>
              {row?.device?.name || 'Unknown Device'}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {row?.device?.deviceType || '-'}
            </Typography>
          </Box>
        </Box>
      ),
    },
    {
      id: 'game',
      label: 'Game',
      minWidth: 200,
      format: (_value, row) => (
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <Box
            sx={{
              width: 40,
              height: 40,
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
            <Typography variant="body2" fontWeight={500}>
              {row?.game?.title || 'Unknown Game'}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {row?.game?.genre || '-'}
            </Typography>
          </Box>
        </Box>
      ),
    },
    {
      id: 'installationDate',
      label: 'Installed On',
      minWidth: 130,
      format: (value) => (value ? formatDisplayDate(value as string) : '-'),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 100,
      align: 'center',
      format: (value) => (
        <Chip
          icon={value ? <CheckCircle fontSize="small" /> : <Cancel fontSize="small" />}
          label={value ? 'Active' : 'Inactive'}
          color={value ? 'success' : 'error'}
          size="small"
        />
      ),
    },
    {
      id: 'createdAt',
      label: 'Assigned On',
      minWidth: 120,
      format: (value) => formatDisplayDate(value as string),
    },
  ];

  const handleDeactivateAssignment = useCallback(
    async (id: string) => {
      try {
        await removeGameFromDevice(id);
        toast.success('Assignment deactivated successfully');
        refetch();
      } catch (_error) {
        toast.error('Failed to deactivate assignment');
      }
    },
    [refetch],
  );

  const handleToggleActive = useCallback(
    async (id: string, currentStatus: boolean) => {
      try {
        await toggleDeviceGameActive(id, !currentStatus);
        toast.success(currentStatus ? 'Game deactivated on device' : 'Game activated on device');
        refetch();
      } catch (_error) {
        toast.error('Failed to update status');
      }
    },
    [refetch],
  );

  const actions: Action<DeviceGameResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Assignment',
      onClick: (row) => handleEditAssignment(row.id),
    },
    {
      icon: <CheckCircle color="success" />,
      label: 'Activate',
      onClick: (row) => handleToggleActive(row.id, row.isActive),
      show: (row) => !row.isActive,
    },
    {
      icon: <Cancel color="warning" />,
      label: 'Deactivate',
      onClick: (row) => handleToggleActive(row.id, row.isActive),
      show: (row) => row.isActive,
    },
    {
      icon: <Delete color="error" />,
      label: 'Deactivate Assignment',
      onClick: (row) => handleDeactivateAssignment(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<DeviceGameResponse>
        title="Device Games"
        description="Manage game assignments to devices. Assign games to specific devices and track installations."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={canWrite ? handleAssignGame : undefined}
        addButtonLabel="Assign Game"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(value === 1 ? `/device-games` : `/device-games?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
