import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Delete, Edit } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deletePlan } from '../../../services/plans/delete';
import { getPlans, type PlanResponse } from '../../../services/plans/list';

export default function PlansPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const planType = searchParams.get('planType') || undefined;
  const isActive = searchParams.get('isActive');

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.PlansWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewPlan = useCallback(() => {
    navigate('/plans/new');
  }, [navigate]);

  const handleEditPlan = useCallback(
    (id: string) => {
      navigate(`/plans/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['plans', debouncedSearch, page, planType, isActive],
    queryFn: () =>
      getPlans({
        search: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        planType: planType as any,
        isActive: isActive === 'true' ? 1 : isActive === 'false' ? 0 : undefined,
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

  const formatPlanType = (type: string) => {
    return type
      .split('_')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  };

  const columns: Column<PlanResponse>[] = [
    {
      id: 'name',
      label: 'Plan Name',
      minWidth: 150,
    },
    {
      id: 'planType',
      label: 'Type',
      minWidth: 120,
      format: (value) => formatPlanType(value as string),
    },
    {
      id: 'price',
      label: 'Price',
      minWidth: 100,
      align: 'right',
      format: (value) => `$${parseFloat(value as string).toFixed(2)}`,
    },
    {
      id: 'validityDays',
      label: 'Validity (Days)',
      minWidth: 100,
      align: 'center',
    },
    {
      id: 'durationMinutes',
      label: 'Duration (Min)',
      minWidth: 100,
      align: 'center',
      format: (value) => (value != null && value !== '' ? String(value) : '-'),
    },
    {
      id: 'maxSessions',
      label: 'Max Sessions',
      minWidth: 100,
      align: 'center',
      format: (value) => (value != null && value !== '' ? String(value) : '-'),
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

  const handleDeactivatePlan = useCallback(
    async (id: string) => {
      try {
        await deletePlan(id);
        toast.success('Plan deactivated successfully');
        refetch();
      } catch (_error) {
        toast.error('Failed to deactivate plan');
      }
    },
    [refetch],
  );

  const actions: Action<PlanResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Plan',
      onClick: (row) => handleEditPlan(row.id),
    },
    {
      icon: <Delete color="error" />,
      label: 'Deactivate Plan',
      onClick: (row) => handleDeactivatePlan(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<PlanResponse>
        title="Plans"
        description="Manage your gaming plans and subscriptions here."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={canWrite ? handleAddNewPlan : undefined}
        addButtonLabel="Add Plan"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) => navigate(value === 1 ? `/plans` : `/plans?page=${value}`)}
        />
      </Box>
    </Box>
  );
}
