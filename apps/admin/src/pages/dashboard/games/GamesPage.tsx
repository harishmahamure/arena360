import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Delete, Edit } from '@mui/icons-material';
import { Avatar, Box, Chip, debounce, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteGame } from '../../../services/game/delete';
import { type GameResponse, getGames } from '../../../services/game/list';

export default function GamesPage() {
  const [inputValue, setInputValue] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.GamesWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['games', debouncedSearch, page],
    queryFn: () =>
      getGames({
        name: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page,
        sortBy: 'sortOrder',
        sortOrder: 'ASC',
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

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await deleteGame(id);
        toast.success('Game removed');
        refetch();
      } catch {
        toast.error('Failed to remove game');
      }
    },
    [refetch],
  );

  const columns: Column<GameResponse>[] = [
    {
      id: 'thumbnailUrl',
      label: 'Cover',
      minWidth: 72,
      format: (value) => (
        <Avatar
          variant="rounded"
          src={(value as string) || undefined}
          alt=""
          sx={{ width: 48, height: 48 }}
        />
      ),
    },
    { id: 'name', label: 'Name', minWidth: 160 },
    { id: 'sortOrder', label: 'Order', minWidth: 80, align: 'right' },
    {
      id: 'videoUrl',
      label: 'Video',
      minWidth: 90,
      align: 'center',
      format: (value) => (
        <Chip label={value ? 'Yes' : 'No'} size="small" color={value ? 'success' : 'default'} />
      ),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 90,
      align: 'center',
      format: (value) => (
        <Chip label={value ? 'Active' : 'Inactive'} color={value ? 'success' : 'error'} />
      ),
    },
  ];

  const actions: Action<GameResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Game',
      onClick: (row) => navigate(`/games/${row.id}`),
    },
    {
      icon: <Delete color="error" />,
      label: 'Remove Game',
      onClick: (row) => handleDelete(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<GameResponse>
        title="Games"
        description="Manage the kiosk game catalog (thumbnails, logos, background videos)."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={canWrite ? () => navigate('/games/new') : undefined}
        addButtonLabel="Add Game"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) => navigate(value === 1 ? '/games' : `/games?page=${value}`)}
        />
      </Box>
    </Box>
  );
}
