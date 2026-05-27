import { type Column, ListViewPage } from '@gaming-cafe/ui';
import { formatCurrency, formatTimeAgo } from '@gaming-cafe/utils';
import { Alert, Box, Chip, debounce, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  type PaymentMethodType,
  PaymentMethodValues,
} from '../../../containers/transactions/schemas/transaction-schema';
import {
  getTransactions,
  PaymentStatus,
  type Transaction,
  TransactionType,
} from '../../../services/transaction/list';

const getStatusColor = (status: PaymentStatus) => {
  switch (status) {
    case PaymentStatus.COMPLETED:
      return 'success';
    case PaymentStatus.PENDING:
      return 'warning';
    case PaymentStatus.FAILED:
      return 'error';
    case PaymentStatus.REFUNDED:
      return 'info';
    case PaymentStatus.CANCELLED:
      return 'default';
    default:
      return 'default';
  }
};

const getPaymentMethodLabel = (method: any) => {
  switch (method) {
    case PaymentMethodValues.CASH:
      return 'Cash';
    case PaymentMethodValues.ONLINE:
      return 'Online';
    case PaymentMethodValues.SPLIT_PAYMENT:
      return 'Split Payment';
    default:
      return method;
  }
};

const columns: Column<Transaction>[] = [
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
      const player = value as Transaction['player'];
      return player?.username || 'N/A';
    },
  },
  {
    id: 'transactionProducts',
    label: 'Products',
    minWidth: 120,
    format: (value) => {
      const products = value as Transaction['transactionProducts'];
      if (!products || products.length === 0) return 'Total only (line items not stored)';
      return products.map((product) => product.product.name).join(', ');
    },
  },
  {
    id: 'amount',
    label: 'Amount',
    minWidth: 100,
    align: 'right',
    format: (value) => {
      return formatCurrency(parseFloat(value as string), 'INR');
    },
  },
  {
    id: 'paymentMethod',
    label: 'Payment',
    minWidth: 100,
    format: (value) => getPaymentMethodLabel(value as PaymentMethodType),
  },
  {
    id: 'paymentStatus',
    label: 'Status',
    minWidth: 100,
    align: 'center',
    format: (value) => (
      <Chip
        label={(value as string).charAt(0).toUpperCase() + (value as string).slice(1)}
        color={getStatusColor(value as PaymentStatus)}
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

export default function TransactionsPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const statusFilter = searchParams.get('status') as PaymentStatus | null;

  const navigate = useNavigate();

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewTransaction = useCallback(() => {
    navigate('/product-transactions/new');
  }, [navigate]);

  const { data, isLoading } = useQuery({
    queryKey: ['product-transactions', debouncedSearch, page, statusFilter],
    queryFn: () =>
      getTransactions({
        page: page,
        ...(statusFilter && { paymentStatus: statusFilter }),
        sortBy: 'transactionDate',
        sortOrder: 'DESC',
        transactionType: TransactionType.PRODUCT_PURCHASE,
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

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <Alert severity="info" sx={{ mb: 2 }}>
        Product transactions currently store the cart total only. Per-product line items are not
        persisted until the transaction_products schema is approved (see docs/adr/DRAFT-0010).
      </Alert>
      <ListViewPage<Transaction>
        title="Transactions"
        description="Manage product purchase transactions here."
        data={data?.data || []}
        columns={columns}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={handleAddNewTransaction}
        actions={[]}
        addButtonLabel="Add Transaction"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(value === 1 ? `/transactions` : `/transactions?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
