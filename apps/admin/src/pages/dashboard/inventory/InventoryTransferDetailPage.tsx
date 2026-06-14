import { FormPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Clear, LocalShipping } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
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
import { useNavigate, useParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveTransferRequest,
  fulfillTransferRequest,
  getInventoryLocations,
  getTransferRequestById,
  rejectTransferRequest,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { formatDisplayDateTime } from '../../../utils/date';
import { formatTransferQuantity } from './transferQuantity';

const statusConfig: Record<
  string,
  { label: string; color: 'warning' | 'success' | 'error' | 'info' | 'default' }
> = {
  pending: { label: 'Pending', color: 'warning' },
  approved: { label: 'Approved', color: 'info' },
  fulfilled: { label: 'Fulfilled', color: 'success' },
  rejected: { label: 'Rejected', color: 'error' },
};

export default function InventoryTransferDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { can } = usePermissions();
  const canFulfill = can(Permission.InventoryTransferFulfill);
  const [rejectOpen, setRejectOpen] = useState(false);
  const [rejectReason, setRejectReason] = useState('');

  const {
    data: transfer,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['transfer-request', id],
    queryFn: () => getTransferRequestById(id as string),
    enabled: !!id,
  });

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const { data: productsData } = useQuery({
    queryKey: ['products-transfer-detail'],
    queryFn: () => getProducts({ limit: 500 }),
    enabled: !!transfer?.lines?.length,
  });

  const productById = useMemo(() => {
    const map = new Map<string, ProductResponse>();
    for (const product of productsData?.data ?? []) {
      map.set(product.id, product);
    }
    return map;
  }, [productsData]);

  const locationName = (locationId: string) =>
    locations?.data.find((l) => l.id === locationId)?.name ?? locationId.slice(0, 8);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['transfer-request', id] });
    void queryClient.invalidateQueries({ queryKey: ['transfer-requests'] });
  };

  const approveMut = useMutation({
    mutationFn: () => approveTransferRequest(id as string),
    onSuccess: () => {
      toastUtils.success('Transfer approved');
      invalidate();
    },
    onError: () => toastUtils.error('Failed to approve'),
  });

  const fulfillMut = useMutation({
    mutationFn: () => fulfillTransferRequest(id as string),
    onSuccess: () => {
      toastUtils.success('Stock sent — transfer fulfilled');
      void queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      invalidate();
    },
    onError: () => toastUtils.error('Failed to fulfill — check warehouse stock'),
  });

  const rejectMut = useMutation({
    mutationFn: (reason: string) => rejectTransferRequest(id as string, reason),
    onSuccess: () => {
      toastUtils.success('Transfer rejected');
      setRejectOpen(false);
      setRejectReason('');
      invalidate();
    },
    onError: () => toastUtils.error('Failed to reject'),
  });

  const status = transfer?.status ?? 'pending';
  const statusChip = statusConfig[status] ?? { label: 'Pending', color: 'warning' as const };
  const actionPending = approveMut.isPending || fulfillMut.isPending || rejectMut.isPending;

  return (
    <>
      <FormPage
        title="Transfer request"
        description={
          transfer
            ? `${locationName(transfer.fromLocationId)} → ${locationName(transfer.toLocationId)}`
            : 'Loading…'
        }
        backTo="/inventory/transfers"
        backLabel="Back to transfers"
        breadcrumbs={[
          { label: 'Inventory', to: '/inventory/transfers' },
          { label: 'Transfer details' },
        ]}
      >
        {error ? (
          <Alert severity="error">Failed to load transfer request.</Alert>
        ) : isLoading || !transfer ? (
          <Typography color="text.secondary">Loading transfer details…</Typography>
        ) : (
          <Stack spacing={3}>
            <Paper variant="outlined" sx={{ p: 3 }}>
              <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} alignItems="flex-start">
                <Box sx={{ flex: 1 }}>
                  <Typography variant="overline" color="text.secondary">
                    Status
                  </Typography>
                  <Box sx={{ mt: 0.5 }}>
                    <Chip label={statusChip.label} color={statusChip.color} size="small" />
                  </Box>
                </Box>
                <Box>
                  <Typography variant="overline" color="text.secondary">
                    Requested
                  </Typography>
                  <Typography>{formatDisplayDateTime(transfer.createdAt)}</Typography>
                </Box>
                {transfer.approvedAt ? (
                  <Box>
                    <Typography variant="overline" color="text.secondary">
                      Approved
                    </Typography>
                    <Typography>{formatDisplayDateTime(transfer.approvedAt)}</Typography>
                  </Box>
                ) : null}
                {transfer.fulfilledAt ? (
                  <Box>
                    <Typography variant="overline" color="text.secondary">
                      Fulfilled
                    </Typography>
                    <Typography>{formatDisplayDateTime(transfer.fulfilledAt)}</Typography>
                  </Box>
                ) : null}
              </Stack>

              {transfer.rejectionReason ? (
                <Alert severity="error" sx={{ mt: 2 }}>
                  Rejected: {transfer.rejectionReason}
                </Alert>
              ) : null}
            </Paper>

            <Paper variant="outlined" sx={{ p: 0, overflow: 'hidden' }}>
              <Box sx={{ px: 3, py: 2, borderBottom: 1, borderColor: 'divider' }}>
                <Typography variant="h6" fontWeight={600}>
                  Line items
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  Quantities stored as pieces; boxes shown when the product has units per box.
                </Typography>
              </Box>
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Product</TableCell>
                    <TableCell align="right">Quantity</TableCell>
                    <TableCell align="right">Pieces</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {(transfer.lines ?? []).map((line) => {
                    const product = productById.get(line.productId);
                    const unitsPerBox = product?.unitsPerPurchaseUnit ?? 1;
                    return (
                      <TableRow key={line.id}>
                        <TableCell>
                          {product?.name ?? line.productId.slice(0, 8)}
                          {product?.sku ? (
                            <Typography variant="caption" color="text.secondary" display="block">
                              SKU: {product.sku}
                            </Typography>
                          ) : null}
                        </TableCell>
                        <TableCell align="right">
                          {formatTransferQuantity(line.quantityPieces, unitsPerBox)}
                        </TableCell>
                        <TableCell align="right">{line.quantityPieces}</TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </Paper>

            {canFulfill ? (
              <Stack direction="row" spacing={2} flexWrap="wrap">
                {status === 'pending' ? (
                  <>
                    <Button
                      variant="contained"
                      color="success"
                      startIcon={<CheckCircleOutline />}
                      disabled={actionPending}
                      onClick={() => approveMut.mutate()}
                    >
                      Approve
                    </Button>
                    <Button
                      variant="outlined"
                      color="error"
                      startIcon={<Clear />}
                      disabled={actionPending}
                      onClick={() => setRejectOpen(true)}
                    >
                      Reject
                    </Button>
                  </>
                ) : null}
                {status === 'approved' ? (
                  <Button
                    variant="contained"
                    startIcon={<LocalShipping />}
                    disabled={actionPending}
                    onClick={() => fulfillMut.mutate()}
                  >
                    Send stock
                  </Button>
                ) : null}
                <Button variant="text" onClick={() => navigate('/inventory/transfers')}>
                  Back to list
                </Button>
              </Stack>
            ) : null}
          </Stack>
        )}
      </FormPage>

      <Dialog open={rejectOpen} onClose={() => setRejectOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Reject transfer request</DialogTitle>
        <DialogContent>
          <TextField
            label="Reason"
            value={rejectReason}
            onChange={(e) => setRejectReason(e.target.value)}
            fullWidth
            multiline
            rows={3}
            sx={{ mt: 1 }}
            required
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRejectOpen(false)}>Cancel</Button>
          <Button
            color="error"
            variant="contained"
            disabled={rejectReason.trim().length < 3 || rejectMut.isPending}
            onClick={() => rejectMut.mutate(rejectReason)}
          >
            Reject
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
