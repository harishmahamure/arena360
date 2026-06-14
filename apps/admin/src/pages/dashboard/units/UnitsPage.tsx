import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils, useAsyncAction } from '@gaming-cafe/utils';
import { Delete, Edit } from '@mui/icons-material';
import { Chip, debounce } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteUnit } from '../../../services/units/delete';
import { getUnits, type UnitResponse } from '../../../services/units/list';
import { buildListUrl } from '../../../utils/buildListUrl';

export default function UnitsPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const isActiveFilter = searchParams.get('active');

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.UnitsWrite);
  const { loading: actionLoading, run } = useAsyncAction();

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewUnit = useCallback(() => {
    navigate('/units/new');
  }, [navigate]);

  const handleEditUnit = useCallback(
    (id: string) => {
      navigate(`/units/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['units', debouncedSearch, page, isActiveFilter],
    queryFn: () =>
      getUnits({
        name: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        isActive: isActiveFilter === 'false' ? false : undefined,
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

  const columns: Column<UnitResponse>[] = [
    {
      id: 'name',
      label: 'Name',
      minWidth: 150,
    },
    {
      id: 'abbreviation',
      label: 'Abbreviation',
      minWidth: 100,
    },
    {
      id: 'type',
      label: 'Type',
      minWidth: 120,
      format: (value) => (value as string).charAt(0).toUpperCase() + (value as string).slice(1),
    },
    {
      id: 'description',
      label: 'Description',
      minWidth: 200,
      format: (value) => (value ? String(value) : '-'),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 80,
      align: 'center',
      format: (value) => (
        <Chip label={value ? 'Active' : 'Inactive'} color={value ? 'success' : 'error'} />
      ),
    },
  ];

  const handleDeactivateUnit = useCallback(
    (id: string) => {
      void run(async () => {
        try {
          await deleteUnit(id);
          toastUtils.success('Unit deactivated successfully');
          refetch();
        } catch (_error) {
          toastUtils.error('Failed to deactivate unit');
        }
      });
    },
    [refetch, run],
  );

  const actions: Action<UnitResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Unit',
      onClick: (row) => handleEditUnit(row.id),
    },
    {
      icon: <Delete color="error" />,
      label: 'Deactivate Unit',
      onClick: (row) => handleDeactivateUnit(row.id),
      disabled: () => actionLoading,
    },
  ];

  return (
    <ListPage<UnitResponse>
      title="Units"
      description="Manage your product measurement units here."
      data={data?.data || []}
      columns={columns}
      actions={canWrite ? actions : []}
      isLoading={isLoading}
      showSearch
      searchValue={inputValue}
      onSearchChange={handleSearch}
      onSearchClear={handleClearSearch}
      onAddClick={canWrite ? handleAddNewUnit : undefined}
      addButtonLabel="Add Unit"
      pagination={{
        page,
        totalPages: data?.totalPages,
        onPageChange: (value) =>
          navigate(buildListUrl('/units', value, { active: isActiveFilter ?? undefined })),
      }}
    />
  );
}
