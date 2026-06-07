import type { UserRole } from '@gaming-cafe/contracts';
import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Block, CheckCircle, Delete, Edit } from '@mui/icons-material';
import {
  Avatar,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  debounce,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deletePlayer } from '../../../services/players/delete';
import { getPlayers, type PlayerResponse } from '../../../services/players/list';
import { updatePlayer } from '../../../services/players/update';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDate } from '../../../utils/date';

const getRoleColor = (role: UserRole) => {
  switch (role) {
    case 'admin':
      return 'error';
    case 'staff':
      return 'warning';
    case 'player':
      return 'primary';
    default:
      return 'default';
  }
};

const getAvatarColor = (role: UserRole) => {
  switch (role) {
    case 'admin':
      return 'error.main';
    case 'staff':
      return 'warning.main';
    default:
      return 'primary.main';
  }
};

const getInitials = (firstName?: string, lastName?: string, username?: string) => {
  if (firstName && lastName) {
    return `${firstName.charAt(0)}${lastName.charAt(0)}`.toUpperCase();
  }
  if (firstName) {
    return firstName.charAt(0).toUpperCase();
  }
  if (username) {
    return username.charAt(0).toUpperCase();
  }
  return 'U';
};

export default function PlayersPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [confirmTarget, setConfirmTarget] = useState<{
    id: string;
    username: string;
    action: 'deactivate' | 'delete';
  } | null>(null);
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const roleFilter = searchParams.get('role') as UserRole | null;
  const activeFilter = searchParams.get('active');

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.PlayersWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewPlayer = useCallback(() => {
    navigate('/players/new');
  }, [navigate]);

  const handleEditPlayer = useCallback(
    (id: string) => {
      navigate(`/players/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['players', debouncedSearch, page, roleFilter, activeFilter],
    queryFn: () =>
      getPlayers({
        username: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        ...(roleFilter && { role: roleFilter }),
        ...(activeFilter !== null && {
          isActive: activeFilter === 'true' ? 1 : 0,
        }),
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

  const columns: Column<PlayerResponse>[] = [
    {
      id: 'username',
      label: 'Player',
      minWidth: 200,
      format: (value, row) => (
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          <Avatar
            sx={{
              bgcolor: getAvatarColor(row?.role ?? 'player'),
              width: 36,
              height: 36,
            }}
          >
            {getInitials(row?.firstName, row?.lastName, row?.username)}
          </Avatar>
          <Box>
            <Typography variant="body2" fontWeight={500}>
              {row?.firstName && row?.lastName
                ? `${row.firstName} ${row.lastName}`
                : (value as string)}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              @{value as string}
            </Typography>
          </Box>
        </Box>
      ),
    },
    {
      id: 'role',
      label: 'Role',
      minWidth: 100,
      align: 'center',
      hideOnMobile: true,
      format: (value) => (
        <Chip
          label={(value as string).charAt(0).toUpperCase() + (value as string).slice(1)}
          color={getRoleColor(value as UserRole)}
          size="small"
          variant="outlined"
        />
      ),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 100,
      align: 'center',
      format: (value) => (
        <Chip
          label={value ? 'Active' : 'Inactive'}
          color={value ? 'success' : 'default'}
          size="small"
        />
      ),
    },
    {
      id: 'createdAt',
      label: 'Joined',
      minWidth: 120,
      hideOnMobile: true,
      format: (value) => formatDisplayDate(value as string),
    },
  ];

  const handleDeactivatePlayer = useCallback(
    async (id: string) => {
      try {
        await deletePlayer(id);
        toastUtils.success('Player deactivated successfully');
        refetch();
      } catch (_error) {
        toastUtils.error('Failed to deactivate player');
      }
    },
    [refetch],
  );

  const handleToggleActive = useCallback(
    async (id: string, currentStatus: boolean) => {
      try {
        await updatePlayer(id, { isActive: !currentStatus });
        toastUtils.success(`Player ${currentStatus ? 'deactivated' : 'activated'} successfully`);
        refetch();
      } catch (_error) {
        toastUtils.error('Failed to update player status');
      }
    },
    [refetch],
  );

  const actions: Action<PlayerResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Player',
      onClick: (row) => handleEditPlayer(row.id),
    },
    {
      icon: <CheckCircle color="success" />,
      label: 'Activate',
      onClick: (row) => handleToggleActive(row.id, row.isActive),
      show: (row) => !row.isActive,
    },
    {
      icon: <Block color="warning" />,
      label: 'Deactivate',
      onClick: (row) =>
        setConfirmTarget({
          id: row.id,
          username: row.username,
          action: 'deactivate',
        }),
      show: (row) => row.isActive,
    },
    {
      icon: <Delete color="error" />,
      label: 'Deactivate Player',
      onClick: (row) =>
        setConfirmTarget({
          id: row.id,
          username: row.username,
          action: 'delete',
        }),
    },
  ];

  return (
    <>
      <ListPage<PlayerResponse>
        title="Players"
        description="Manage your game zone players and admins here."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        showSearch
        searchValue={inputValue}
        onSearchChange={handleSearch}
        onSearchClear={handleClearSearch}
        onAddClick={canWrite ? handleAddNewPlayer : undefined}
        addButtonLabel="Add Player"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(
              buildListUrl('/players', value, {
                role: roleFilter ?? undefined,
                active: activeFilter ?? undefined,
              }),
            ),
        }}
      />

      <Dialog open={confirmTarget !== null} onClose={() => setConfirmTarget(null)}>
        <DialogTitle>
          {confirmTarget?.action === 'delete' ? 'Deactivate player?' : 'Deactivate account?'}
        </DialogTitle>
        <DialogContent>
          <Typography>
            {confirmTarget?.action === 'delete'
              ? `This will deactivate @${confirmTarget.username}. They will no longer be able to sign in or start sessions.`
              : `Deactivate @${confirmTarget?.username}? They will not be able to sign in until reactivated.`}
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmTarget(null)}>Cancel</Button>
          <Button
            color="error"
            variant="contained"
            onClick={async () => {
              if (!confirmTarget) return;
              if (confirmTarget.action === 'delete') {
                await handleDeactivatePlayer(confirmTarget.id);
              } else {
                await handleToggleActive(confirmTarget.id, true);
              }
              setConfirmTarget(null);
            }}
          >
            Deactivate
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
