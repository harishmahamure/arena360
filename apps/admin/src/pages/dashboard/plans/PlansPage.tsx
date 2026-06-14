import type { PlanTypeValue } from '@gaming-cafe/contracts';
import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils, useAsyncAction } from '@gaming-cafe/utils';
import { Delete, Edit } from '@mui/icons-material';
import { Chip, debounce } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deletePlan } from '../../../services/plans/delete';
import { getPlans, type PlanResponse } from '../../../services/plans/list';
import { buildListUrl } from '../../../utils/buildListUrl';

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
  const { loading: actionLoading, run } = useAsyncAction();

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
        planType: planType as PlanTypeValue | undefined,
        isActive: isActive === 'true' ? true : isActive === 'false' ? false : undefined,
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

  const formatSchedule = (row: PlanResponse) => {
    const parts: string[] = [];
    if (row.allowedDays?.length) {
      parts.push(row.allowedDays.map((d) => d.slice(0, 3)).join(', '));
    }
    if (row.allowedMonths?.length) {
      const monthNames = [
        'Jan',
        'Feb',
        'Mar',
        'Apr',
        'May',
        'Jun',
        'Jul',
        'Aug',
        'Sep',
        'Oct',
        'Nov',
        'Dec',
      ];
      parts.push(row.allowedMonths.map((m) => monthNames[m - 1]).join(', '));
    }
    if (row.timeWindowStart && row.timeWindowEnd) {
      parts.push(`${row.timeWindowStart} - ${row.timeWindowEnd}`);
    }
    return parts.length ? parts.join(' | ') : 'All';
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
      id: 'timeCredits',
      label: 'Credits (Min)',
      minWidth: 100,
      align: 'center',
      format: (value) => (value != null ? String(value) : '-'),
    },
    {
      id: 'validityDays',
      label: 'Validity (Days)',
      minWidth: 100,
      align: 'center',
    },
    {
      id: 'allowedDays',
      label: 'Schedule',
      minWidth: 160,
      format: (_value, row) => formatSchedule(row),
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
    (id: string) => {
      void run(async () => {
        try {
          await deletePlan(id);
          toastUtils.success('Plan deactivated successfully');
          refetch();
        } catch (_error) {
          toastUtils.error('Failed to deactivate plan');
        }
      });
    },
    [refetch, run],
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
      disabled: () => actionLoading,
    },
  ];

  return (
    <ListPage<PlanResponse>
      title="Plans"
      description="Manage your gaming plans and subscriptions here."
      data={data?.data || []}
      columns={columns}
      actions={canWrite ? actions : []}
      isLoading={isLoading}
      showSearch
      searchValue={inputValue}
      onSearchChange={handleSearch}
      onSearchClear={handleClearSearch}
      onAddClick={canWrite ? handleAddNewPlan : undefined}
      addButtonLabel="Add Plan"
      pagination={{
        page,
        totalPages: data?.totalPages,
        onPageChange: (value) =>
          navigate(
            buildListUrl('/plans', value, {
              planType,
              isActive: isActive ?? undefined,
            }),
          ),
      }}
    />
  );
}
