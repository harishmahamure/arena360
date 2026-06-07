import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { type Column, ListPage } from '@gaming-cafe/ui';
import { formatCurrency, formatTimeAgo } from '@gaming-cafe/utils';
import { Visibility } from '@mui/icons-material';
import { Chip, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback } from 'react';
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
import { buildListUrl } from '../../../utils/buildListUrl';

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
    case PaymentStatus.CREDIT:
      return 'secondary';
    default:
      return 'default';
  }
};

const getPaymentMethodLabel = (method: PaymentMethodType | string) => {
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
    hideOnMobile: true,
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
    id: 'amount',
    label: 'Amount',
    minWidth: 100,
    align: 'right',
    format: (value) => formatCurrency(parseFloat(value as string), 'INR'),
  },
  {
    id: 'paymentMethod',
    label: 'Payment',
    minWidth: 100,
    hideOnMobile: true,
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
        color={getStatusColor(value as PaymentStatusValue)}
        size="small"
      />
    ),
  },
  {
    id: 'transactionDate',
    label: 'Date',
    minWidth: 120,
    hideOnMobile: true,
    format: (value) => formatTimeAgo(value as string),
  },
];

export default function TransactionsPage() {
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const statusFilter = searchParams.get('status') as PaymentStatusValue | null;

  const navigate = useNavigate();

  const handleAddNewTransaction = useCallback(() => {
    navigate('/product-transactions/new');
  }, [navigate]);

  const { data, isLoading } = useQuery({
    queryKey: ['product-transactions', page, statusFilter],
    queryFn: () =>
      getTransactions({
        page: page,
        ...(statusFilter && { paymentStatus: statusFilter }),
        sortBy: 'transactionDate',
        sortOrder: 'DESC',
        transactionType: TransactionType.PRODUCT_PURCHASE,
      }),
  });

  return (
    <ListPage<Transaction>
      title="POS sales"
      description="Product purchases and snack sales at the counter."
      data={data?.data || []}
      columns={columns}
      isLoading={isLoading}
      showSearch={false}
      onAddClick={handleAddNewTransaction}
      actions={[
        {
          icon: <Visibility fontSize="small" />,
          label: 'View',
          onClick: (row) => navigate(`/product-transactions/${row.id}`),
        },
      ]}
      addButtonLabel="Sell items"
      pagination={{
        page,
        totalPages: data?.totalPages,
        onPageChange: (value) =>
          navigate(
            buildListUrl('/product-transactions', value, {
              status: statusFilter ?? undefined,
            }),
          ),
      }}
    />
  );
}
