import { ActivityTimeline, OperationsShell, StatusBadge, type StatusTone } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';
import { usePermissions } from '../../../hooks/usePermissions';
import {
  approvePurchaseOrder,
  cancelPurchaseOrder,
  getPurchaseOrder,
  markPurchaseOrderOrdered,
  type PurchaseOrderStatus,
  receivePurchaseOrder,
  rejectPurchaseOrder,
  submitPurchaseOrder,
} from '../../../services/inventory';
import { formatDisplayDateTime } from '../../../utils/date';

const currency = new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' });
const tones: Record<PurchaseOrderStatus, StatusTone> = {
  draft: 'default',
  submitted: 'warning',
  approved: 'info',
  rejected: 'error',
  ordered: 'info',
  partially_received: 'warning',
  received: 'success',
  cancelled: 'error',
};

export default function PurchaseOrderDetailPage() {
  const { id = '' } = useParams();
  const { isAdmin } = usePermissions();
  const client = useQueryClient();
  const [receiveOpen, setReceiveOpen] = useState(false);
  const [invoice, setInvoice] = useState('');
  const [paymentMethod, setPaymentMethod] = useState('cash');
  const [paymentAccount, setPaymentAccount] = useState('');
  const [accepted, setAccepted] = useState<Record<string, number>>({});
  const order = useQuery({
    queryKey: ['purchase-order', id],
    queryFn: () => getPurchaseOrder(id),
    enabled: Boolean(id),
  });
  const refresh = () => {
    void client.invalidateQueries({ queryKey: ['purchase-order', id] });
    void client.invalidateQueries({ queryKey: ['purchase-orders'] });
    void client.invalidateQueries({ queryKey: ['inventory-overview'] });
  };
  const action = useMutation({
    mutationFn: async (kind: string) => {
      if (kind === 'submit') return submitPurchaseOrder(id);
      if (kind === 'approve') return approvePurchaseOrder(id);
      if (kind === 'reject') {
        const reason = window.prompt('Reason for rejection');
        if (!reason) throw new Error('A rejection reason is required');
        return rejectPurchaseOrder(id, reason);
      }
      if (kind === 'ordered') return markPurchaseOrderOrdered(id);
      return cancelPurchaseOrder(id);
    },
    onSuccess: () => {
      toastUtils.success('Purchase order updated');
      refresh();
    },
    onError: (error: unknown) =>
      toastUtils.error(error instanceof Error ? error.message : 'Action failed'),
  });
  const receive = useMutation({
    mutationFn: () =>
      receivePurchaseOrder(id, {
        invoiceReference: invoice,
        paymentMethod,
        paymentAccount: paymentAccount || undefined,
        lines: Object.entries(accepted)
          .filter(([, boxes]) => boxes > 0)
          .map(([purchaseOrderLineId, acceptedBoxes]) => ({ purchaseOrderLineId, acceptedBoxes })),
      }),
    onSuccess: () => {
      toastUtils.success('Receipt finalized; stock and expense posted');
      setReceiveOpen(false);
      refresh();
    },
    onError: (error: unknown) =>
      toastUtils.error(error instanceof Error ? error.message : 'Receipt failed'),
  });
  const data = order.data;
  const totalAccepted = useMemo(
    () => Object.values(accepted).reduce((sum, quantity) => sum + quantity, 0),
    [accepted],
  );
  if (order.isError)
    return (
      <Alert severity="error" sx={{ m: 4 }}>
        Purchase order could not be loaded.
      </Alert>
    );
  if (!data)
    return (
      <OperationsShell header={{ title: 'Purchase order' }}>
        <Typography>Loading…</Typography>
      </OperationsShell>
    );

  const actions = (
    <Stack direction="row" gap={1} flexWrap="wrap" justifyContent="flex-end">
      {['draft', 'rejected'].includes(data.status) ? (
        <Button variant="contained" onClick={() => action.mutate('submit')}>
          Submit for approval
        </Button>
      ) : null}
      {isAdmin && data.status === 'submitted' ? (
        <>
          <Button variant="contained" color="success" onClick={() => action.mutate('approve')}>
            Approve
          </Button>
          <Button color="error" onClick={() => action.mutate('reject')}>
            Reject
          </Button>
        </>
      ) : null}
      {isAdmin && data.status === 'approved' ? (
        <Button variant="contained" onClick={() => action.mutate('ordered')}>
          Mark ordered
        </Button>
      ) : null}
      {['approved', 'ordered', 'partially_received'].includes(data.status) ? (
        <Button
          variant="contained"
          onClick={() => {
            setAccepted(Object.fromEntries(data.lines.map((line) => [line.id, 0])));
            setReceiveOpen(true);
          }}
        >
          Receive delivery
        </Button>
      ) : null}
      {isAdmin && ['draft', 'submitted', 'approved', 'ordered'].includes(data.status) ? (
        <Button color="error" onClick={() => action.mutate('cancel')}>
          Cancel PO
        </Button>
      ) : null}
    </Stack>
  );
  const timeline = [
    { id: 'created', title: 'Draft created', at: formatDisplayDateTime(data.createdAt) },
    ...(data.submittedAt
      ? [
          {
            id: 'submitted',
            title: 'Submitted for approval',
            at: formatDisplayDateTime(data.submittedAt),
          },
        ]
      : []),
    ...(data.approvedAt
      ? [
          {
            id: 'approved',
            title: data.status === 'rejected' ? 'Rejected' : 'Approved',
            detail: data.rejectionReason ?? undefined,
            at: formatDisplayDateTime(data.approvedAt),
          },
        ]
      : []),
    ...(data.orderedAt
      ? [{ id: 'ordered', title: 'Marked ordered', at: formatDisplayDateTime(data.orderedAt) }]
      : []),
  ];

  return (
    <>
      <OperationsShell
        header={{
          title: data.poNumber,
          description: `Version ${data.version}`,
          backTo: '/inventory/purchase-orders',
          breadcrumbs: [
            { label: 'Inventory', to: '/inventory' },
            { label: 'Purchase orders', to: '/inventory/purchase-orders' },
            { label: data.poNumber },
          ],
        }}
        actions={actions}
      >
        <Stack spacing={2}>
          <Paper variant="outlined" sx={{ p: 2.5 }}>
            <Stack direction={{ xs: 'column', sm: 'row' }} justifyContent="space-between" gap={2}>
              <Box>
                <Typography variant="overline">Status</Typography>
                <Box>
                  <StatusBadge label={data.status} tone={tones[data.status]} />
                </Box>
              </Box>
              <Box>
                <Typography variant="overline">Expected delivery</Typography>
                <Typography>{data.expectedDeliveryDate ?? 'Not set'}</Typography>
              </Box>
              <Box textAlign={{ sm: 'right' }}>
                <Typography variant="overline">Order total</Typography>
                <Typography variant="h5" fontWeight={700}>
                  {currency.format(data.total)}
                </Typography>
              </Box>
            </Stack>
          </Paper>
          <Paper variant="outlined" sx={{ overflowX: 'auto' }}>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>Product</TableCell>
                  <TableCell align="right">Ordered</TableCell>
                  <TableCell align="right">Received</TableCell>
                  <TableCell align="right">Units / box</TableCell>
                  <TableCell align="right">Cost / box</TableCell>
                  <TableCell align="right">Line total</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {data.lines.map((line) => (
                  <TableRow key={line.id}>
                    <TableCell>
                      <Typography fontWeight={600}>{line.productName}</Typography>
                      <Typography variant="caption" color="text.secondary">
                        {line.productSku ?? 'No SKU'}
                      </Typography>
                    </TableCell>
                    <TableCell align="right">{line.orderedBoxes}</TableCell>
                    <TableCell align="right">{line.receivedBoxes}</TableCell>
                    <TableCell align="right">{line.unitsPerBoxSnapshot}</TableCell>
                    <TableCell align="right">{currency.format(line.boxCostSnapshot)}</TableCell>
                    <TableCell align="right">{currency.format(line.lineTotal)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Paper>
          <Paper variant="outlined" sx={{ p: 2.5 }}>
            <Typography variant="h6" mb={2}>
              Activity
            </Typography>
            <ActivityTimeline items={timeline} />
          </Paper>
        </Stack>
      </OperationsShell>
      <Dialog open={receiveOpen} onClose={() => setReceiveOpen(false)} maxWidth="md" fullWidth>
        <DialogTitle>Receive delivery</DialogTitle>
        <DialogContent>
          <Alert severity="info" sx={{ mb: 2 }}>
            Finalizing creates stock movements and exactly one approved expense for the accepted
            value.
          </Alert>
          <Stack spacing={2}>
            <TextField
              label="Vendor invoice reference"
              value={invoice}
              onChange={(event) => setInvoice(event.target.value)}
              required
            />
            <Stack direction={{ xs: 'column', sm: 'row' }} gap={2}>
              <TextField
                select
                label="Payment method"
                value={paymentMethod}
                onChange={(event) => setPaymentMethod(event.target.value)}
                fullWidth
              >
                <MenuItem value="cash">Cash</MenuItem>
                <MenuItem value="online">Online / bank</MenuItem>
              </TextField>
              <TextField
                label="Payment account / reference"
                value={paymentAccount}
                onChange={(event) => setPaymentAccount(event.target.value)}
                fullWidth
                required={paymentMethod !== 'cash'}
              />
            </Stack>
            {data.lines.map((line) => {
              const outstanding = line.orderedBoxes - line.receivedBoxes;
              return (
                <TextField
                  key={line.id}
                  label={`${line.productName} — accepted boxes (max ${outstanding})`}
                  type="number"
                  value={accepted[line.id] ?? 0}
                  onChange={(event) =>
                    setAccepted((current) => ({
                      ...current,
                      [line.id]: Math.min(outstanding, Math.max(0, Number(event.target.value))),
                    }))
                  }
                  slotProps={{ htmlInput: { min: 0, max: outstanding } }}
                />
              );
            })}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setReceiveOpen(false)}>Back</Button>
          <Button
            variant="contained"
            disabled={
              !invoice.trim() ||
              totalAccepted < 1 ||
              receive.isPending ||
              (paymentMethod !== 'cash' && !paymentAccount.trim())
            }
            onClick={() => receive.mutate()}
          >
            Finalize receipt
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
