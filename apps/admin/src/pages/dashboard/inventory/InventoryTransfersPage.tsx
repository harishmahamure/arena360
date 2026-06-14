import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Clear, LocalShipping } from '@mui/icons-material';
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveTransferRequest,
  fulfillTransferRequest,
  getInventoryLocations,
  getTransferRequests,
  rejectTransferRequest,
  type StockTransferRequest,
} from '../../../services/inventory';
import { buildListUrl } from '../../../utils/buildListUrl';
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
      toastUtils.success('Transfer approved');
      refetch();
    },
    onError: () => toastUtils.error('Failed to approve'),
  });

  const fulfillMut = useMutation({
    mutationFn: (id: string) => fulfillTransferRequest(id),
    onSuccess: () => {
      toastUtils.success('Stock sent — transfer fulfilled');
      queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      refetch();
    },
    onError: () => toastUtils.error('Failed to fulfill — check warehouse stock'),
  });

  const rejectMut = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) =>
      rejectTransferRequest(id, reason),
    onSuccess: () => {
      toastUtils.success('Transfer rejected');
      setRejectId(null);
      setRejectReason('');
      refetch();
    },
    onError: () => toastUtils.error('Failed to reject'),
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
            disabled: (row: StockTransferRequest) =>
              row.status !== 'pending' || approveMut.isPending || fulfillMut.isPending,
          },
          {
            label: 'Send stock',
            icon: <LocalShipping fontSize="small" />,
            color: 'primary' as const,
            onClick: (row: StockTransferRequest) => fulfillMut.mutate(row.id),
            disabled: (row: StockTransferRequest) =>
              row.status !== 'approved' || approveMut.isPending || fulfillMut.isPending,
          },
          {
            label: 'Reject',
            icon: <Clear fontSize="small" />,
            color: 'error' as const,
            onClick: (row: StockTransferRequest) => setRejectId(row.id),
            disabled: (row: StockTransferRequest) =>
              row.status !== 'pending' || approveMut.isPending || fulfillMut.isPending,
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
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load transfers
        </Alert>
      )}
      <ListPage
        title="Stock Transfer Requests"
        description="Store requests stock from warehouse; admin approves and sends"
        columns={columns}
        data={filtered}
        actions={actions}
        isLoading={isLoading}
        showSearch
        searchValue={search}
        onSearchChange={(e) => setSearch(e.target.value)}
        onSearchClear={() => setSearch('')}
        onAddClick={
          can(Permission.InventoryTransferRequest)
            ? () => navigate('/inventory/transfers/new')
            : undefined
        }
        addButtonLabel="New Request"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(buildListUrl('/inventory/transfers', value, { status: statusFilter })),
        }}
      />

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
    </>
  );
}
