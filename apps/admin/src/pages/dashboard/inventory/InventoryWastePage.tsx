import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { CheckCircleOutline, Clear } from '@mui/icons-material';
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
import { useMutation, useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  approveWasteEvent,
  getInventoryLocations,
  getWasteEvents,
  rejectWasteEvent,
  type StockWasteEvent,
} from '../../../services/inventory';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDate } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'warning' | 'success' | 'error' }> = {
  pending: { label: 'Pending', color: 'warning' },
  approved: { label: 'Approved', color: 'success' },
  rejected: { label: 'Rejected', color: 'error' },
};

export default function InventoryWastePage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const statusFilter = searchParams.get('status') || undefined;
  const page = Number(searchParams.get('page') || '1');
  const { can } = usePermissions();
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
    queryKey: ['waste-events', page, statusFilter],
    queryFn: () => getWasteEvents({ page, limit: 20, status: statusFilter }),
  });

  const approveMut = useMutation({
    mutationFn: (id: string) => approveWasteEvent(id),
    onSuccess: () => {
      toastUtils.success('Waste approved — stock deducted');
      refetch();
    },
    onError: () => toastUtils.error('Failed to approve waste'),
  });

  const rejectMut = useMutation({
    mutationFn: ({ id, reason }: { id: string; reason: string }) => rejectWasteEvent(id, reason),
    onSuccess: () => {
      toastUtils.success('Waste rejected');
      setRejectId(null);
      setRejectReason('');
      refetch();
    },
    onError: () => toastUtils.error('Failed to reject'),
  });

  const columns: Column<StockWasteEvent>[] = [
    {
      id: 'locationId',
      label: 'Location',
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
      id: 'notes',
      label: 'Notes',
      minWidth: 160,
      format: (v) => (
        <Typography variant="body2" noWrap sx={{ maxWidth: 200 }}>
          {(v as string) || '—'}
        </Typography>
      ),
    },
    {
      id: 'createdAt',
      label: 'Recorded',
      minWidth: 140,
      format: (v) => formatDisplayDate(v as string),
    },
  ];

  const actions: Action<StockWasteEvent>[] = [
    ...(can(Permission.InventoryWasteApprove)
      ? [
          {
            label: 'Approve',
            icon: <CheckCircleOutline fontSize="small" />,
            color: 'success' as const,
            onClick: (row: StockWasteEvent) => approveMut.mutate(row.id),
            disabled: (row: StockWasteEvent) => row.status !== 'pending',
          },
          {
            label: 'Reject',
            icon: <Clear fontSize="small" />,
            color: 'error' as const,
            onClick: (row: StockWasteEvent) => setRejectId(row.id),
            disabled: (row: StockWasteEvent) => row.status !== 'pending',
          },
        ]
      : []),
  ];

  const filtered =
    data?.data.filter((r) =>
      `${locationName(r.locationId)} ${r.notes ?? ''}`.toLowerCase().includes(search.toLowerCase()),
    ) ?? [];

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load waste events
        </Alert>
      )}
      <ListPage
        title="Stock Waste"
        description="Spoiled, damaged, or expired stock write-offs"
        columns={columns}
        data={filtered}
        actions={actions}
        isLoading={isLoading}
        showSearch
        searchValue={search}
        onSearchChange={(e) => setSearch(e.target.value)}
        onSearchClear={() => setSearch('')}
        onAddClick={
          can(Permission.InventoryWasteRecord) ? () => navigate('/inventory/waste/new') : undefined
        }
        addButtonLabel="Record waste"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(buildListUrl('/inventory/waste', value, { status: statusFilter })),
        }}
      />

      <Dialog open={!!rejectId} onClose={() => setRejectId(null)} maxWidth="sm" fullWidth>
        <DialogTitle>Reject waste record</DialogTitle>
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
