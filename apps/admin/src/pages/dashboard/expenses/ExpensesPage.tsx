import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Add } from '@mui/icons-material';
import { Box, Button, Chip, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveExpense,
  type Expense,
  getExpenses,
  rejectExpense,
} from '../../../services/expenses';
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
      toast.success('Expense approved');
      refetch();
    } catch {
      toast.error('Failed to approve expense');
    }
  };

  const handleReject = async (expense: Expense) => {
    const reason = prompt('Rejection reason:');
    if (!reason) return;
    try {
      await rejectExpense(expense.id, reason);
      toast.success('Expense rejected');
      refetch();
    } catch {
      toast.error('Failed to reject expense');
    }
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
        return <Chip label={config.label} color={config.color} size="small" />;
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
      onClick: (row) => navigate(`/expenses/${row.id}`),
    },
    ...(can(Permission.ExpensesApprove)
      ? [
          {
            label: 'Approve',
            onClick: (row: Expense) => handleApprove(row),
            disabled: (row: Expense) => row.approvalStatus !== 'pending',
          },
          {
            label: 'Reject',
            onClick: (row: Expense) => handleReject(row),
            disabled: (row: Expense) => row.approvalStatus !== 'pending',
          },
        ]
      : []),
  ];

  return (
    <Box>
      <ListViewPage
        title="Expenses"
        subtitle="Track and manage business expenses"
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        error={error ? 'Failed to load expenses' : undefined}
        headerAction={
          can(Permission.ExpensesWrite) ? (
            <Button
              variant="contained"
              startIcon={<Add />}
              onClick={() => navigate('/expenses/new')}
            >
              Add Expense
            </Button>
          ) : undefined
        }
      />
      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) =>
              navigate(`/expenses?page=${p}${statusFilter ? `&status=${statusFilter}` : ''}`)
            }
          />
        </Box>
      )}
    </Box>
  );
}
