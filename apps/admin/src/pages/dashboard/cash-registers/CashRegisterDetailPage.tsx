import { type Column, DataGrid, DetailPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Edit } from '@mui/icons-material';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Grid,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useParams } from 'react-router-dom';
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
    error: fetchError,
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

  const status = register
    ? (statusConfig[register.status] ?? { label: 'Unknown', color: 'default' as const })
    : undefined;

  return (
    <>
      <DetailPage
        title="Cash register"
        description={register ? `Opened ${formatDisplayDateTime(register.createdAt)}` : undefined}
        backTo="/cash-registers"
        backLabel="Back to cash registers"
        breadcrumbs={[
          { label: 'Cash registers', to: '/cash-registers' },
          { label: 'Register details' },
        ]}
        isLoading={isLoading}
        error={!isLoading && (fetchError || !register) ? 'Cash register not found' : null}
        onRetry={() => void queryClient.invalidateQueries({ queryKey: ['cash-register', id] })}
        status={status}
        summary={
          register ? (
            <>
              <Grid container spacing={2}>
                {[
                  { label: 'Opening', value: formatCurrency(register.openingBalance) },
                  { label: 'Cash in', value: formatCurrency(register.totalCashIn ?? 0) },
                  { label: 'Cash out', value: formatCurrency(register.totalCashOut ?? 0) },
                  { label: 'Deposited', value: formatCurrency(register.totalDeposited ?? 0) },
                  {
                    label: 'Expected closing',
                    value:
                      register.expectedClosing != null
                        ? formatCurrency(register.expectedClosing)
                        : '—',
                  },
                  {
                    label: 'Closing',
                    value:
                      register.closingBalance != null
                        ? formatCurrency(register.closingBalance)
                        : '—',
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
                  <Grid key={item.label} size={{ xs: 6, sm: 4, md: 3 }}>
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
            </>
          ) : undefined
        }
        actions={
          register && isAdmin ? (
            <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
              {register.status === 'closed' && (
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
              {register.status === 'open' && (
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
            </Stack>
          ) : undefined
        }
        sections={
          register
            ? [
                {
                  title: 'Entries',
                  description: 'Cash movements recorded against this register',
                  content: (
                    <DataGrid<CashRegisterEntry>
                      columns={entryColumns}
                      data={register.entries ?? []}
                      rowKey={(row) => row.id}
                      showActionsLabel={false}
                      emptyMessage="No entries"
                    />
                  ),
                },
              ]
            : undefined
        }
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
    </>
  );
}
