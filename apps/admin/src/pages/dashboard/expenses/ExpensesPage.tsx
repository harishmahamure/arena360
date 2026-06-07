import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Clear, Visibility } from '@mui/icons-material';
import { Alert, Chip, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { approveExpense, type Expense, getExpenses } from '../../../services/expenses';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDate } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'warning' | 'success' | 'error' }> = {
  pending: { label: 'Pending', color: 'warning' },
  approved: { label: 'Approved', color: 'success' },
  rejected: { label: 'Rejected', color: 'error' },
};

export default function ExpensesPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');
  const statusFilter = searchParams.get('status') || undefined;
  const { can } = usePermissions();

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['expenses', page, statusFilter],
    queryFn: () =>
      getExpenses({
        page,
        approvalStatus: statusFilter,
        sortBy: 'expenseDate',
        sortOrder: 'DESC',
      }),
  });

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const handleApprove = async (expense: Expense) => {
    try {
      await approveExpense(expense.id);
      toastUtils.success('Expense approved');
      refetch();
    } catch {
      toastUtils.error('Failed to approve expense');
    }
  };

  const handleReject = (expense: Expense) => {
    navigate(`/expenses/${expense.id}`);
  };

  const columns: Column<Expense>[] = [
    {
      id: 'description',
      label: 'Description',
      minWidth: 200,
      format: (value) => (
        <Typography variant="body2" noWrap sx={{ maxWidth: 250 }}>
          {(value as string) || 'No description'}
        </Typography>
      ),
    },
    {
      id: 'amount',
      label: 'Amount',
      minWidth: 120,
      format: (value) => (
        <Typography variant="body2" fontWeight={600}>
          {formatCurrency(value as number)}
        </Typography>
      ),
    },
    {
      id: 'paymentMethod',
      label: 'Payment',
      minWidth: 100,
      format: (value) => <Chip label={value as string} size="small" variant="outlined" />,
    },
    {
      id: 'approvalStatus',
      label: 'Status',
      minWidth: 100,
      format: (value) => {
        const config = statusConfig[value as string] || statusConfig.pending;
        return (
          <Chip
            label={config?.label || 'Unknown'}
            color={config?.color || 'default'}
            size="small"
          />
        );
      },
    },
    {
      id: 'expenseDate',
      label: 'Date',
      minWidth: 120,
      format: (value) => formatDisplayDate(value as string),
    },
  ];

  const actions: Action<Expense>[] = [
    {
      label: 'View',
      icon: <Visibility fontSize="small" />,
      onClick: (row) => navigate(`/expenses/${row.id}`),
    },
    ...(can(Permission.ExpensesApprove)
      ? [
          {
            label: 'Approve',
            icon: <CheckCircleOutline fontSize="small" />,
            color: 'success' as const,
            onClick: (row: Expense) => handleApprove(row),
            disabled: (row: Expense) => row.approvalStatus !== 'pending',
          },
          {
            label: 'Reject',
            icon: <Clear fontSize="small" />,
            color: 'error' as const,
            onClick: (row: Expense) => handleReject(row),
            disabled: (row: Expense) => row.approvalStatus !== 'pending',
          },
        ]
      : []),
  ];

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load expenses
        </Alert>
      )}
      <ListPage<Expense>
        title="Expenses"
        description="Track and manage business expenses"
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        showSearch={false}
        onAddClick={can(Permission.ExpensesWrite) ? () => navigate('/expenses/new') : undefined}
        addButtonLabel="Add Expense"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(buildListUrl('/expenses', value, { status: statusFilter })),
        }}
      />
    </>
  );
}
