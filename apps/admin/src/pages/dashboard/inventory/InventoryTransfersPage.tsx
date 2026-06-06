import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
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
  Pagination,
  TextField,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveTransferRequest,
  fulfillTransferRequest,
  getInventoryLocations,
  getTransferRequests,
  rejectTransferRequest,
  type StockTransferRequest,
} from '../../../services/inventory';
import { formatDisplayDate } from '../../../utils/date';

const statusConfig: Record<
  string,
  { label: string; color: 'warning' | 'success' | 'error' | 'info' | 'default' }
> = {
  pending: { label: 'Pending', color: 'warning' },
  approved: { label: 'Approved', color: 'info' },
  fulfilled: { label: 'Fulfilled', color: 'success' },
  rejected: { label: 'Rejected', color: 'error' },
};

export default function InventoryTransfersPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const statusFilter = searchParams.get('status') || undefined;
  const page = Number(searchParams.get('page') || '1');
  const { can } = usePermissions();
  const queryClient = useQueryClient();
  const [rejectId, setRejectId] = useState<string | null>(null);
  const [rejectReason, setRejectReason] = useState('');
  const [search, setSearch] = useState('');

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const locationName = (id: string) =>
    locations?.data.find((l) => l.id === id)?.name ?? id.slice(0, 8);

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['transfer-requests', page, statusFilter],
    queryFn: () =>
      getTransferRequests({
        page,
        limit: 20,
        status: statusFilter,
      }),
  });

  const approveMut = useMutation({
    mutationFn: (id: string) => approveTransferRequest(id),
    onSuccess: () => {
      toast.success('Transfer approved');
      refetch();
    },
    onError: () => toast.error('Failed to approve'),
  });

  const fulfillMut = useMutation({
    mutationFn: (id: string) => fulfillTransferRequest(id),
    onSuccess: () => {
      toast.success('Stock sent — transfer fulfilled');
      queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      refetch();
    },
    onError: () => toast.error('Failed to fulfill — check warehouse stock'),
  });

  const rejectMut = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) =>
      rejectTransferRequest(id, reason),
    onSuccess: () => {
      toast.success('Transfer rejected');
      setRejectId(null);
      setRejectReason('');
      refetch();
    },
    onError: () => toast.error('Failed to reject'),
  });

  const columns: Column<StockTransferRequest>[] = [
    {
      id: 'fromLocationId',
      label: 'From',
      minWidth: 120,
      format: (v) => locationName(v as string),
    },
    {
      id: 'toLocationId',
      label: 'To',
      minWidth: 120,
      format: (v) => locationName(v as string),
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 100,
      format: (v) => {
        const cfg = statusConfig[v as string] ?? statusConfig.pending;
        return <Chip label={cfg?.label} color={cfg?.color} size="small" />;
      },
    },
    {
      id: 'createdAt',
      label: 'Requested',
      minWidth: 140,
      format: (v) => formatDisplayDate(v as string),
    },
  ];

  const actions: Action<StockTransferRequest>[] = [
    ...(can(Permission.InventoryTransferFulfill)
      ? [
          {
            label: 'Approve',
            icon: <CheckCircleOutline fontSize="small" />,
            color: 'success' as const,
            onClick: (row: StockTransferRequest) => approveMut.mutate(row.id),
            disabled: (row: StockTransferRequest) => row.status !== 'pending',
          },
          {
            label: 'Send stock',
            icon: <LocalShipping fontSize="small" />,
            color: 'primary' as const,
            onClick: (row: StockTransferRequest) => fulfillMut.mutate(row.id),
            disabled: (row: StockTransferRequest) => row.status !== 'approved',
          },
          {
            label: 'Reject',
            icon: <Clear fontSize="small" />,
            color: 'error' as const,
            onClick: (row: StockTransferRequest) => setRejectId(row.id),
            disabled: (row: StockTransferRequest) => row.status !== 'pending',
          },
        ]
      : []),
  ];

  const filtered =
    data?.data.filter((r) =>
      `${locationName(r.fromLocationId)} ${locationName(r.toLocationId)}`
        .toLowerCase()
        .includes(search.toLowerCase()),
    ) ?? [];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          Failed to load transfers
        </Alert>
      )}
      <ListViewPage
        title="Stock Transfer Requests"
        description="Store requests stock from warehouse; admin approves and sends"
        columns={columns}
        data={filtered}
        actions={actions}
        isLoading={isLoading}
        inputValue={search}
        handleSearch={(e) => setSearch(e.target.value)}
        handleClearSearch={() => setSearch('')}
        onAddClick={
          can(Permission.InventoryTransferRequest)
            ? () => navigate('/inventory/transfers/new')
            : undefined
        }
        addButtonLabel="New Request"
      />

      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) =>
              navigate(
                `/inventory/transfers?page=${p}${statusFilter ? `&status=${statusFilter}` : ''}`,
              )
            }
          />
        </Box>
      )}

      <Dialog open={!!rejectId} onClose={() => setRejectId(null)} maxWidth="sm" fullWidth>
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
          <Button onClick={() => setRejectId(null)}>Cancel</Button>
          <Button
            color="error"
            variant="contained"
            disabled={rejectReason.trim().length < 3 || rejectMut.isPending}
            onClick={() => rejectId && rejectMut.mutate({ id: rejectId, reason: rejectReason })}
          >
            Reject
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
