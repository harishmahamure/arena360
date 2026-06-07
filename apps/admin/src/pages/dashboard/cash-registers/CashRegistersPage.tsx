import { type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Edit, Visibility } from '@mui/icons-material';
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  type CashRegister,
  getCashRegisters,
  reconcileRegister,
  updateOpeningBalance,
} from '../../../services/cash-registers';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDateTime } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'info' }> = {
  open: { label: 'Open', color: 'success' },
  closed: { label: 'Closed', color: 'default' },
  reconciled: { label: 'Reconciled', color: 'info' },
};

export default function CashRegistersPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');
  const queryClient = useQueryClient();

  const [reconcileOpen, setReconcileOpen] = useState(false);
  const [reconcileTarget, setReconcileTarget] = useState<CashRegister | null>(null);
  const [reconcileNotes, setReconcileNotes] = useState('');
  const [reconcileLoading, setReconcileLoading] = useState(false);

  const [balanceOpen, setBalanceOpen] = useState(false);
  const [balanceTarget, setBalanceTarget] = useState<CashRegister | null>(null);
  const [openingBalance, setOpeningBalance] = useState('');
  const [balanceLoading, setBalanceLoading] = useState(false);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const { data, isLoading, error } = useQuery({
    queryKey: ['cash-registers', page],
    queryFn: () => getCashRegisters({ page }),
  });

  const columns: Column<CashRegister>[] = [
    {
      id: 'openingBalance',
      label: 'Opening',
      minWidth: 110,
      format: (value) => (
        <Typography variant="body2" fontWeight={600}>
          {formatCurrency(value as number)}
        </Typography>
      ),
    },
    {
      id: 'totalCashIn',
      label: 'Cash In',
      minWidth: 110,
      format: (value) => (
        <Typography variant="body2" fontWeight={600} color="success.main">
          {formatCurrency((value as number) ?? 0)}
        </Typography>
      ),
    },
    {
      id: 'totalDeposited',
      label: 'Deposited',
      minWidth: 110,
      format: (value) => (
        <Typography variant="body2" fontWeight={600} color="info.main">
          {formatCurrency((value as number) ?? 0)}
        </Typography>
      ),
    },
    {
      id: 'expectedClosing',
      label: 'Expected',
      minWidth: 110,
      format: (value) =>
        value != null ? (
          <Typography variant="body2" fontWeight={600}>
            {formatCurrency(value as number)}
          </Typography>
        ) : (
          '-'
        ),
    },
    {
      id: 'closingBalance',
      label: 'Closing',
      minWidth: 110,
      format: (value) =>
        value != null ? (
          <Typography variant="body2" fontWeight={600}>
            {formatCurrency(value as number)}
          </Typography>
        ) : (
          '-'
        ),
    },
    {
      id: 'variance',
      label: 'Variance',
      minWidth: 110,
      format: (value) => {
        if (value == null) return '-';
        const v = value as number;
        return (
          <Typography
            variant="body2"
            fontWeight={600}
            color={v === 0 ? 'success.main' : 'error.main'}
          >
            {formatCurrency(v)}
          </Typography>
        );
      },
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 100,
      format: (value) => {
        const config = statusConfig[value as string] || statusConfig.closed;
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
      id: 'createdAt',
      label: 'Opened At',
      minWidth: 160,
      format: (value) => formatDisplayDateTime(value as string),
    },
  ];

  const handleReconcile = async () => {
    if (!reconcileTarget) return;
    setReconcileLoading(true);
    try {
      await reconcileRegister(reconcileTarget.id, {
        reconciliationNotes: reconcileNotes || undefined,
      });
      toastUtils.success('Cash register reconciled');
      setReconcileOpen(false);
      queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to reconcile');
    } finally {
      setReconcileLoading(false);
    }
  };

  const handleSetBalance = async () => {
    if (!balanceTarget) return;
    setBalanceLoading(true);
    try {
      await updateOpeningBalance(balanceTarget.id, { openingBalance: Number(openingBalance) });
      toastUtils.success('Opening balance updated');
      setBalanceOpen(false);
      queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to update balance');
    } finally {
      setBalanceLoading(false);
    }
  };

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load cash registers
        </Alert>
      )}
      <ListPage<CashRegister>
        title="Cash Registers"
        description="Cash register history and reconciliation"
        columns={columns}
        data={data?.data ?? []}
        showSearch={false}
        actions={[
          {
            label: 'View',
            icon: <Visibility fontSize="small" />,
            onClick: (row) => navigate(`/cash-registers/${row.id}`),
          },
          {
            label: 'Reconcile',
            icon: <CheckCircleOutline fontSize="small" />,
            color: 'success',
            show: (row) => row.status === 'closed',
            onClick: (row) => {
              setReconcileTarget(row);
              setReconcileNotes('');
              setReconcileOpen(true);
            },
          },
          {
            label: 'Set Balance',
            icon: <Edit fontSize="small" />,
            color: 'warning',
            show: (row) => row.status === 'open',
            onClick: (row) => {
              setBalanceTarget(row);
              setOpeningBalance(row.openingBalance.toString());
              setBalanceOpen(true);
            },
          },
        ]}
        isLoading={isLoading}
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) => navigate(buildListUrl('/cash-registers', value, {})),
        }}
      />

      <Dialog open={reconcileOpen} onClose={() => setReconcileOpen(false)} fullWidth maxWidth="sm">
        <DialogTitle>Reconcile Cash Register</DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 2 }}>
            Are you sure you want to mark this cash register as reconciled?
          </Typography>
          <TextField
            fullWidth
            label="Reconciliation Notes (Optional)"
            multiline
            rows={3}
            value={reconcileNotes}
            onChange={(e) => setReconcileNotes(e.target.value)}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setReconcileOpen(false)} disabled={reconcileLoading}>
            Cancel
          </Button>
          <Button
            onClick={handleReconcile}
            variant="contained"
            color="success"
            disabled={reconcileLoading}
          >
            Reconcile
          </Button>
        </DialogActions>
      </Dialog>

      <Dialog open={balanceOpen} onClose={() => setBalanceOpen(false)} fullWidth maxWidth="sm">
        <DialogTitle>Set Opening Balance</DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 2 }}>
            Update the opening balance for this active cash register.
          </Typography>
          <TextField
            fullWidth
            label="Opening Balance"
            type="number"
            value={openingBalance}
            onChange={(e) => setOpeningBalance(e.target.value)}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setBalanceOpen(false)} disabled={balanceLoading}>
            Cancel
          </Button>
          <Button
            onClick={handleSetBalance}
            variant="contained"
            color="primary"
            disabled={balanceLoading}
          >
            Save
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
