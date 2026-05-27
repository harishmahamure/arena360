import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Delete, Edit } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { deleteGame } from '../../../services/games/delete';
import { type GameResponse, getGames } from '../../../services/games/list';

export default function GamesPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const deletedGames = searchParams.get('deleted') === 'true' ? 0 : undefined;
  const category = searchParams.get('category') || undefined;
  const platform = searchParams.get('platform') || undefined;
  const isMultiplayer = searchParams.get('multiplayer') === 'true' ? 1 : undefined;

  const navigate = useNavigate();

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewGame = useCallback(() => {
    navigate('/games/new');
  }, [navigate]);

  const handleEditGame = useCallback(
    (id: string) => {
      navigate(`/games/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['games', debouncedSearch, page, deletedGames, category, platform, isMultiplayer],
    queryFn: () =>
      getGames({
        title: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        isActive: deletedGames,
        category,
        platform,
        isMultiplayer,
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

  const columns: Column<GameResponse>[] = [
    {
      id: 'title',
      label: 'Title',
      minWidth: 180,
    },
    {
      id: 'genre',
      label: 'Genre',
      minWidth: 100,
      format: (value) =>
        value ? (value as string).charAt(0).toUpperCase() + (value as string).slice(1) : '-',
    },
    {
      id: 'category',
      label: 'Category',
      minWidth: 100,
      format: (value) =>
        value ? (value as string).charAt(0).toUpperCase() + (value as string).slice(1) : '-',
    },
    {
      id: 'platform',
      label: 'Platform',
      minWidth: 120,
      format: (value) => (value as string) || '-',
    },
    {
      id: 'developer',
      label: 'Developer',
      minWidth: 120,
      format: (value) => (value as string) || '-',
    },
    {
      id: 'isMultiplayer',
      label: 'Multiplayer',
      minWidth: 100,
      align: 'center',
      format: (value) => (
        <Chip label={value ? 'Yes' : 'No'} color={value ? 'primary' : 'default'} size="small" />
      ),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 80,
      align: 'center',
      format: (value) => (
        <Chip
          label={value ? 'Active' : 'Inactive'}
          color={value ? 'success' : 'error'}
          size="small"
        />
      ),
    },
  ];

  const handleDeleteGame = useCallback(
    async (id: string) => {
      try {
        await deleteGame(id);
        toast.success('Game deleted successfully');
        refetch();
      } catch (_error) {
        toast.error('Failed to delete game');
      }
    },
    [refetch],
  );

  const actions: Action<GameResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Game',
      onClick: (row) => handleEditGame(row.id),
    },
    {
      icon: <Delete color="error" />,
      label: 'Delete Game',
      onClick: (row) => handleDeleteGame(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<GameResponse>
        title="Games"
        description="Manage your game zone Games here."
        data={data?.data || []}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={handleAddNewGame}
        addButtonLabel="Add Game"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) => navigate(value === 1 ? `/games` : `/games?page=${value}`)}
        />
      </Box>
    </Box>
  );
}
