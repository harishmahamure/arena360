import { type Column, FormSkeleton, ListViewPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Edit } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  GridLegacy as Grid,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { PageHeader } from '../../../components/PageHeader';
import { usePermissions } from '../../../hooks/usePermissions';
import {
  type CashRegisterEntry,
  getCashRegister,
  reconcileRegister,
  updateOpeningBalance,
} from '../../../services/cash-registers';
import { formatDisplayDateTime } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'info' }> = {
  open: { label: 'Open', color: 'success' },
  closed: { label: 'Closed', color: 'default' },
  reconciled: { label: 'Reconciled', color: 'info' },
};

const entryTypeLabels: Record<string, string> = {
  cash_in: 'Cash in',
  cash_out: 'Cash out',
};

export default function CashRegisterDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { isAdmin } = usePermissions();

  const [reconcileOpen, setReconcileOpen] = useState(false);
  const [reconcileNotes, setReconcileNotes] = useState('');
  const [reconcileLoading, setReconcileLoading] = useState(false);

  const [balanceOpen, setBalanceOpen] = useState(false);
  const [openingBalance, setOpeningBalance] = useState('');
  const [balanceLoading, setBalanceLoading] = useState(false);

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const {
    data: register,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['cash-register', id],
    queryFn: () => getCashRegister(id as string),
    enabled: !!id,
  });

  const handleReconcile = async () => {
    if (!id) return;
    setReconcileLoading(true);
    try {
      await reconcileRegister(id, { reconciliationNotes: reconcileNotes || undefined });
      toastUtils.success('Cash register reconciled');
      setReconcileOpen(false);
      void queryClient.invalidateQueries({ queryKey: ['cash-register', id] });
      void queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to reconcile');
    } finally {
      setReconcileLoading(false);
    }
  };

  const handleSetBalance = async () => {
    if (!id) return;
    setBalanceLoading(true);
    try {
      await updateOpeningBalance(id, { openingBalance: Number(openingBalance) });
      toastUtils.success('Opening balance updated');
      setBalanceOpen(false);
      void queryClient.invalidateQueries({ queryKey: ['cash-register', id] });
      void queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to update balance');
    } finally {
      setBalanceLoading(false);
    }
  };

  const entryColumns: Column<CashRegisterEntry>[] = [
    {
      id: 'entryType',
      label: 'Type',
      minWidth: 100,
      format: (value) => entryTypeLabels[value as string] ?? (value as string),
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
      id: 'reason',
      label: 'Reason',
      minWidth: 180,
      format: (value) => (value as string) || '—',
    },
    {
      id: 'createdAt',
      label: 'Time',
      minWidth: 160,
      format: (value) => formatDisplayDateTime(value as string),
    },
  ];

  if (isLoading) {
    return (
      <Box sx={{ px: 4, py: 3 }}>
        <FormSkeleton />
      </Box>
    );
  }

  if (error || !register) {
    return (
      <Box sx={{ px: 4, py: 3 }}>
        <Alert severity="error">Cash register not found</Alert>
        <Button onClick={() => navigate('/cash-registers')} sx={{ mt: 2 }}>
          Back to cash registers
        </Button>
      </Box>
    );
  }

  const status = statusConfig[register.status] ?? {
    label: 'Unknown',
    color: 'default' as const,
  };

  return (
    <Box sx={{ px: 4, py: 3 }}>
      <PageHeader
        title="Cash register"
        description={`Opened ${formatDisplayDateTime(register.createdAt)}`}
        backTo="/cash-registers"
        backLabel="Back to cash registers"
        breadcrumbs={[
          { label: 'Cash registers', to: '/cash-registers' },
          { label: 'Register details' },
        ]}
      />
      <Chip label={status.label} color={status.color} size="small" sx={{ mb: 3 }} />

      <Card variant="outlined" sx={{ mb: 3 }}>
        <CardContent>
          <Grid container spacing={2}>
            {[
              { label: 'Opening', value: formatCurrency(register.openingBalance) },
              { label: 'Cash in', value: formatCurrency(register.totalCashIn ?? 0) },
              { label: 'Cash out', value: formatCurrency(register.totalCashOut ?? 0) },
              { label: 'Deposited', value: formatCurrency(register.totalDeposited ?? 0) },
              {
                label: 'Expected closing',
                value:
                  register.expectedClosing != null ? formatCurrency(register.expectedClosing) : '—',
              },
              {
                label: 'Closing',
                value:
                  register.closingBalance != null ? formatCurrency(register.closingBalance) : '—',
              },
              {
                label: 'Variance',
                value: register.variance != null ? formatCurrency(register.variance) : '—',
                color:
                  register.variance != null && register.variance !== 0
                    ? 'error.main'
                    : 'success.main',
              },
            ].map((item) => (
              <Grid item xs={6} sm={4} md={3} key={item.label}>
                <Typography variant="caption" color="text.secondary">
                  {item.label}
                </Typography>
                <Typography variant="body1" fontWeight={600} color={item.color}>
                  {item.value}
                </Typography>
              </Grid>
            ))}
          </Grid>

          {register.notes && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 2 }}>
              Notes: {register.notes}
            </Typography>
          )}

          <Box sx={{ display: 'flex', gap: 1, mt: 2, flexWrap: 'wrap' }}>
            {isAdmin && register.status === 'closed' && (
              <Button
                variant="outlined"
                color="success"
                startIcon={<CheckCircleOutline />}
                onClick={() => {
                  setReconcileNotes('');
                  setReconcileOpen(true);
                }}
              >
                Reconcile
              </Button>
            )}
            {isAdmin && register.status === 'open' && (
              <Button
                variant="outlined"
                color="warning"
                startIcon={<Edit />}
                onClick={() => {
                  setOpeningBalance(register.openingBalance.toString());
                  setBalanceOpen(true);
                }}
              >
                Set opening balance
              </Button>
            )}
          </Box>
        </CardContent>
      </Card>

      <ListViewPage<CashRegisterEntry>
        title="Entries"
        description="Cash movements recorded against this register"
        columns={entryColumns}
        data={register.entries ?? []}
        actions={[]}
        isLoading={false}
        inputValue=""
        handleSearch={() => {}}
        handleClearSearch={() => {}}
        showSearch={false}
      />

      <Dialog open={reconcileOpen} onClose={() => setReconcileOpen(false)} fullWidth maxWidth="sm">
        <DialogTitle>Reconcile Cash Register</DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 2 }}>
            Mark this cash register as reconciled?
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
            onClick={() => void handleReconcile()}
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
          <TextField
            fullWidth
            label="Opening Balance"
            type="number"
            value={openingBalance}
            onChange={(e) => setOpeningBalance(e.target.value)}
            sx={{ mt: 1 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setBalanceOpen(false)} disabled={balanceLoading}>
            Cancel
          </Button>
          <Button
            onClick={() => void handleSetBalance()}
            variant="contained"
            disabled={balanceLoading}
          >
            Save
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
