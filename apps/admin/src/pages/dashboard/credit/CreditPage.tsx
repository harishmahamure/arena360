import { type Action, type Column, CurrencyField, DataGrid, GridSkeleton } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Payment as PaymentIcon } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  InputLabel,
  MenuItem,
  Pagination,
  Select,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  type PaymentMethodType,
  PaymentMethodValues,
  paymentMethodOptions,
} from '../../../containers/transactions/schemas/transaction-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  type CreditPlayerRow,
  getCreditAccounts,
  getPlayerCredit,
  type OutstandingTxn,
  settleCredit,
} from '../../../services/credit';

interface SettlementLine {
  transactionId: string;
  transactionType: string;
  remaining: number;
  selected: boolean;
  amount: string;
}

export default function CreditPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');
  const { can } = usePermissions();
  const canSettle = can(Permission.CreditWrite);

  const [settlePlayer, setSettlePlayer] = useState<CreditPlayerRow | null>(null);
  const [lines, setLines] = useState<SettlementLine[]>([]);
  const [paymentMethod, setPaymentMethod] = useState<PaymentMethodType>(PaymentMethodValues.CASH);
  const [cashAmount, setCashAmount] = useState('');
  const [onlineAmount, setOnlineAmount] = useState('');
  const [notes, setNotes] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [dialogError, setDialogError] = useState<string | undefined>();

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['credit-accounts', page],
    queryFn: () =>
      getCreditAccounts({
        page,
        sortBy: 'outstanding',
        sortOrder: 'DESC',
      }),
  });

  const {
    data: playerCredit,
    isLoading: detailLoading,
    error: detailError,
  } = useQuery({
    queryKey: ['player-credit-detail', settlePlayer?.playerId],
    queryFn: () => {
      if (!settlePlayer?.playerId) throw new Error('No player selected');
      return getPlayerCredit(settlePlayer.playerId);
    },
    enabled: !!settlePlayer,
  });

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const openSettlement = (row: CreditPlayerRow) => {
    setSettlePlayer(row);
    setLines([]);
    setPaymentMethod(PaymentMethodValues.CASH);
    setCashAmount('');
    setOnlineAmount('');
    setNotes('');
    setDialogError(undefined);
  };

  const closeSettlement = () => {
    setSettlePlayer(null);
    setLines([]);
    setDialogError(undefined);
  };

  useEffect(() => {
    if (playerCredit?.transactions) {
      setLines(
        playerCredit.transactions.map((txn: OutstandingTxn) => ({
          transactionId: txn.transactionId,
          transactionType: txn.transactionType,
          remaining: txn.remaining,
          selected: false,
          amount: txn.remaining.toFixed(2),
        })),
      );
    }
  }, [playerCredit]);

  const selectedTotal = lines
    .filter((l) => l.selected)
    .reduce((sum, l) => sum + (Number.parseFloat(l.amount) || 0), 0);

  const toggleLine = (transactionId: string, checked: boolean) => {
    setLines((prev) =>
      prev.map((l) => (l.transactionId === transactionId ? { ...l, selected: checked } : l)),
    );
  };

  const updateLineAmount = (transactionId: string, amount: string) => {
    setLines((prev) => prev.map((l) => (l.transactionId === transactionId ? { ...l, amount } : l)));
  };

  const selectAll = () => {
    setLines((prev) => prev.map((l) => ({ ...l, selected: true, amount: l.remaining.toFixed(2) })));
  };

  const handleSettle = async () => {
    if (!settlePlayer) return;
    setDialogError(undefined);

    const items = lines
      .filter((l) => l.selected)
      .map((l) => ({
        transactionId: l.transactionId,
        amount: Number.parseFloat(l.amount),
      }))
      .filter((l) => l.amount > 0);

    if (items.length === 0) {
      setDialogError('Select at least one transaction to settle');
      return;
    }

    for (const item of items) {
      const line = lines.find((l) => l.transactionId === item.transactionId);
      if (line && item.amount > line.remaining + 0.001) {
        setDialogError(
          `Amount exceeds remaining for transaction ${item.transactionId.slice(0, 8)}`,
        );
        return;
      }
    }

    setSubmitting(true);
    try {
      await settleCredit({
        playerId: settlePlayer.playerId,
        items,
        paymentMethod,
        cashAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? Number.parseFloat(cashAmount)
            : paymentMethod === PaymentMethodValues.CASH
              ? selectedTotal
              : undefined,
        onlineAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? Number.parseFloat(onlineAmount)
            : paymentMethod === PaymentMethodValues.ONLINE
              ? selectedTotal
              : undefined,
        notes: notes || undefined,
      });
      toastUtils.success('Credit bill settled');
      closeSettlement();
      refetch();
      queryClient.invalidateQueries({ queryKey: ['player-credit'] });
      queryClient.invalidateQueries({ queryKey: ['credit-settlements'] });
    } catch (err) {
      setDialogError(err instanceof Error ? err.message : 'Settlement failed');
    } finally {
      setSubmitting(false);
    }
  };

  const columns: Column<CreditPlayerRow>[] = [
    {
      id: 'username',
      label: 'Member',
      minWidth: 140,
      format: (_v, row) => {
        const name =
          row.firstName && row.lastName ? `${row.firstName} ${row.lastName}` : row.username;
        return (
          <Box>
            <Typography variant="body2" fontWeight={600}>
              {name}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              @{row.username}
            </Typography>
          </Box>
        );
      },
    },
    {
      id: 'outstanding',
      label: 'Outstanding',
      minWidth: 120,
      format: (v) => (
        <Typography variant="body2" fontWeight={600} color="error.main">
          {formatCurrency(v as number)}
        </Typography>
      ),
    },
    {
      id: 'creditLimit',
      label: 'Limit',
      minWidth: 100,
      format: (v) => formatCurrency(v as number),
    },
    {
      id: 'available',
      label: 'Available',
      minWidth: 100,
      format: (v) => formatCurrency(v as number),
    },
  ];

  const actions: Action<CreditPlayerRow>[] = canSettle
    ? [
        {
          label: 'Settle',
          icon: <PaymentIcon fontSize="small" />,
          onClick: (row) => openSettlement(row),
        },
      ]
    : [];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <Box
        sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', mb: 2 }}
      >
        <Box>
          <Typography variant="h4" sx={{ mb: 0 }}>
            Credit Members
          </Typography>
          <Typography variant="body1">Members with outstanding credit (tab / khata)</Typography>
        </Box>
        <Button variant="outlined" onClick={() => navigate('/credit/settlements')}>
          Settlement history
        </Button>
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          {error instanceof Error ? error.message : 'Failed to load credit accounts'}
        </Alert>
      )}

      {isLoading ? (
        <GridSkeleton />
      ) : (
        <DataGrid
          columns={columns}
          data={data?.data ?? []}
          actions={actions}
          rowKey={(row) => row.playerId}
          onRowClick={(row) => canSettle && openSettlement(row)}
          emptyMessage="No members with outstanding credit"
        />
      )}

      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) => navigate(`/credit?page=${p}`)}
          />
        </Box>
      )}

      <Dialog open={!!settlePlayer} onClose={closeSettlement} maxWidth="md" fullWidth>
        <DialogTitle>Settle credit — {settlePlayer?.username}</DialogTitle>
        <DialogContent>
          {detailLoading && (
            <Alert severity="info" sx={{ mb: 2 }}>
              Loading outstanding transactions…
            </Alert>
          )}
          {detailError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {detailError instanceof Error ? detailError.message : 'Failed to load credit details'}
            </Alert>
          )}
          {playerCredit?.summary && (
            <Alert severity="info" sx={{ mb: 2 }}>
              Outstanding: {formatCurrency(playerCredit.summary.outstanding)} · Limit:{' '}
              {formatCurrency(playerCredit.summary.creditLimit)}
            </Alert>
          )}
          {dialogError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {dialogError}
            </Alert>
          )}

          {lines.length > 0 && (
            <>
              <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 1 }}>
                <Button size="small" onClick={selectAll}>
                  Select all (full remaining)
                </Button>
              </Box>
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell padding="checkbox" />
                    <TableCell>Type</TableCell>
                    <TableCell align="right">Remaining</TableCell>
                    <TableCell align="right">Pay now</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {lines.map((line) => (
                    <TableRow key={line.transactionId}>
                      <TableCell padding="checkbox">
                        <Checkbox
                          checked={line.selected}
                          onChange={(e) => toggleLine(line.transactionId, e.target.checked)}
                        />
                      </TableCell>
                      <TableCell>
                        {line.transactionType.replace('_', ' ')}
                        <Typography variant="caption" display="block" color="text.secondary">
                          {line.transactionId.slice(0, 8)}…
                        </Typography>
                      </TableCell>
                      <TableCell align="right">{formatCurrency(line.remaining)}</TableCell>
                      <TableCell align="right">
                        <CurrencyField
                          size="small"
                          value={line.amount}
                          disabled={!line.selected}
                          onChange={(e) => updateLineAmount(line.transactionId, e.target.value)}
                          sx={{ width: 100 }}
                        />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}

          {!detailLoading && lines.length === 0 && !detailError && (
            <Alert severity="success">No outstanding transactions for this member.</Alert>
          )}

          <Box sx={{ mt: 3, display: 'grid', gap: 2, gridTemplateColumns: '1fr 1fr' }}>
            <FormControl fullWidth size="small">
              <InputLabel>Payment method</InputLabel>
              <Select
                value={paymentMethod}
                label="Payment method"
                onChange={(e) => setPaymentMethod(e.target.value as PaymentMethodType)}
              >
                {paymentMethodOptions
                  .filter((o) => o.value !== PaymentMethodValues.CREDIT)
                  .map((o) => (
                    <MenuItem key={o.value} value={o.value}>
                      {o.label}
                    </MenuItem>
                  ))}
              </Select>
            </FormControl>
            <TextField
              size="small"
              label="Notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              fullWidth
            />
            {paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (
              <>
                <CurrencyField
                  size="small"
                  label="Cash amount"
                  value={cashAmount}
                  onChange={(e) => setCashAmount(e.target.value)}
                />
                <CurrencyField
                  size="small"
                  label="Online amount"
                  value={onlineAmount}
                  onChange={(e) => setOnlineAmount(e.target.value)}
                />
              </>
            )}
          </Box>

          <Typography variant="body2" sx={{ mt: 2 }} fontWeight={600}>
            Settlement total: {formatCurrency(selectedTotal)}
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeSettlement}>Cancel</Button>
          <Button
            variant="contained"
            onClick={handleSettle}
            disabled={submitting || selectedTotal <= 0 || !canSettle}
          >
            {submitting ? 'Settling…' : 'Settle selected'}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
