import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { capitalize, formatCurrency, formatTimeAgo } from '@gaming-cafe/utils';
import { Visibility } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useEnrichedTransactions } from '../../../hooks/useEnrichedTransactions';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import type { PaymentMethod } from '../../../services/transaction/list';
import {
  getTransactions,
  PaymentStatus,
  type TransactionResponse,
  TransactionType,
} from '../../../services/transaction/list';

const getStatusColor = (status: PaymentStatusValue) => {
  switch (status) {
    case PaymentStatus.COMPLETED:
      return 'success';
    case PaymentStatus.PENDING:
      return 'warning';
    case PaymentStatus.FAILED:
      return 'error';
    case PaymentStatus.REFUNDED:
      return 'info';
    default:
      return 'default';
  }
};

const getPaymentMethodLabel = (method: PaymentMethod) => {
  return capitalize(method);
};

const formatPlanType = (type: string) => {
  return type
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
};

const columns: Column<TransactionResponse>[] = [
  {
    id: 'id',
    label: 'Transaction ID',
    minWidth: 120,
    format: (value) => (
      <Typography
        variant="body2"
        sx={{
          fontFamily: 'monospace',
          fontSize: '0.75rem',
        }}
      >
        {(value as string).slice(0, 8)}...
      </Typography>
    ),
  },
  {
    id: 'player',
    label: 'Player',
    minWidth: 150,
    format: (value) => {
      const player = value as TransactionResponse['player'];
      return player?.username || 'N/A';
    },
  },
  {
    id: 'plan',
    label: 'Plan',
    minWidth: 150,
    format: (value) => {
      const plan = value as TransactionResponse['plan'];
      return plan?.name || 'N/A';
    },
  },
  {
    id: 'plan',
    label: 'Plan Type',
    minWidth: 120,
    format: (value) => {
      const plan = value as TransactionResponse['plan'];
      return plan?.planType ? formatPlanType(plan.planType) : 'N/A';
    },
  },
  {
    id: 'amount',
    label: 'Amount',
    minWidth: 100,
    align: 'right',
    format: (value) => formatCurrency(Number(value), 'INR'),
  },
  {
    id: 'paymentMethod',
    label: 'Payment',
    minWidth: 100,
    format: (value) => getPaymentMethodLabel(value as PaymentMethod),
  },
  {
    id: 'paymentStatus',
    label: 'Status',
    minWidth: 100,
    align: 'center',
    format: (value) => (
      <Chip
        label={(value as string).charAt(0).toUpperCase() + (value as string).slice(1)}
        color={getStatusColor(value as PaymentStatusValue)}
        size="small"
      />
    ),
  },
  {
    id: 'transactionDate',
    label: 'Date',
    minWidth: 120,
    format: (value) => formatTimeAgo(value as string),
  },
];

export default function PlanTransactionsPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const statusFilter = searchParams.get('status') as PaymentStatusValue | null;

  const navigate = useNavigate();

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewTransaction = useCallback(() => {
    navigate('/plan-transactions/new');
  }, [navigate]);

  const handleViewTransaction = useCallback(
    (id: string) => {
      navigate(`/plan-transactions/${id}`);
    },
    [navigate],
  );

  const { data, isLoading } = useQuery({
    queryKey: ['plan-transactions', debouncedSearch, page, statusFilter],
    queryFn: () =>
      getTransactions({
        page: page,
        transactionType: TransactionType.PLAN_PURCHASE,
        ...(statusFilter && { paymentStatus: statusFilter }),
      }),
  });

  const enrichedTransactions = useEnrichedTransactions(data?.data);
  const { can } = usePermissions();

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

  const actions: Action<TransactionResponse>[] = [
    {
      icon: <Visibility color="info" />,
      label: 'View Transaction',
      onClick: (row) => handleViewTransaction(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<TransactionResponse>
        title="Plan Transactions"
        description="Manage plan purchase transactions and assignments here."
        data={enrichedTransactions}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={can(Permission.PlayerPlansWrite) ? handleAddNewTransaction : undefined}
        addButtonLabel="New Transaction"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(value === 1 ? `/plan-transactions` : `/plan-transactions?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
