import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Cancel, Check } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Pagination,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveDeposit,
  type CashDeposit,
  getCashDeposits,
  rejectDeposit,
} from '../../../services/cash-deposits';
import { formatDisplayDate } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'warning' | 'success' | 'error' }> = {
  pending: { label: 'Pending', color: 'warning' },
  approved: { label: 'Approved', color: 'success' },
  rejected: { label: 'Rejected', color: 'error' },
};

export default function CashDepositsPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');
  const statusFilter = searchParams.get('status') || undefined;
  const { can, isAdmin } = usePermissions();
  const [approveTarget, setApproveTarget] = useState<CashDeposit | null>(null);
  const [depositType, setDepositType] = useState<'bank' | 'home'>('bank');
  const [inputValue, setInputValue] = useState('');

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['cash-deposits', page, statusFilter],
    queryFn: () =>
      getCashDeposits({
        page,
        status: statusFilter,
        sortBy: 'createdAt',
        sortOrder: 'DESC',
      }),
  });

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const handleApprove = async () => {
    if (!approveTarget) return;
    try {
      await approveDeposit(approveTarget.id, depositType);
      toast.success(`Deposit approved for ${depositType}`);
      setApproveTarget(null);
      refetch();
    } catch {
      toast.error('Failed to approve deposit');
    }
  };

  const handleReject = async (deposit: CashDeposit) => {
    const reason = prompt('Rejection reason:');
    if (!reason) return;
    try {
      await rejectDeposit(deposit.id, reason);
      toast.success('Deposit rejected');
      refetch();
    } catch {
      toast.error('Failed to reject deposit');
    }
  };

  const columns: Column<CashDeposit>[] = [
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
      id: 'status',
      label: 'Status',
      minWidth: 100,
      format: (value) => {
        const config = statusConfig[value as string] ?? statusConfig.pending;
        return <Chip label={config?.label ?? ''} color={config?.color ?? 'warning'} size="small" />;
      },
    },
    {
      id: 'depositType',
      label: 'Destination',
      minWidth: 100,
      format: (value) => (value ? String(value) : '—'),
    },
    {
      id: 'initiatedBy',
      label: 'Initiated By',
      minWidth: 140,
      format: (value) => String(value).slice(0, 8),
    },
    {
      id: 'createdAt',
      label: 'Created',
      minWidth: 140,
      format: (value) => formatDisplayDate(value as string),
    },
  ];

  const actions: Action<CashDeposit>[] = isAdmin
    ? [
        {
          label: 'Approve',
          icon: <Check color="success" />,
          onClick: (row) => {
            setDepositType('bank');
            setApproveTarget(row);
          },
          disabled: (row) => row.status !== 'pending',
        },
        {
          label: 'Reject',
          icon: <Cancel />,
          onClick: (row) => handleReject(row),
          disabled: (row) => row.status !== 'pending',
        },
      ]
    : [];

  if (!can(Permission.CashDepositsRead)) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>You do not have permission to view cash deposits.</Typography>
      </Box>
    );
  }

  return (
    <Box>
      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          Failed to load cash deposits
        </Alert>
      )}
      <ListViewPage<CashDeposit>
        title="Cash Deposits"
        description="Review and approve staff cash deposit withdrawals."
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={(event) => setInputValue(event.target.value)}
        handleClearSearch={() => setInputValue('')}
      />
      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) =>
              navigate(`/cash-deposits?page=${p}${statusFilter ? `&status=${statusFilter}` : ''}`)
            }
          />
        </Box>
      )}

      <Dialog open={Boolean(approveTarget)} onClose={() => setApproveTarget(null)}>
        <DialogTitle>Approve Cash Deposit</DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 2 }}>
            Verify the physical cash denominations, then select where the admin will take the cash.
          </Typography>
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Button
              variant={depositType === 'bank' ? 'contained' : 'outlined'}
              onClick={() => setDepositType('bank')}
            >
              Bank Deposit
            </Button>
            <Button
              variant={depositType === 'home' ? 'contained' : 'outlined'}
              onClick={() => setDepositType('home')}
            >
              Take Home
            </Button>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setApproveTarget(null)}>Cancel</Button>
          <Button variant="contained" onClick={handleApprove}>
            Approve
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
