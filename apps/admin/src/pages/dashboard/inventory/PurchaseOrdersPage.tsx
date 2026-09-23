import { type Column, ListPage, StatusBadge, type StatusTone } from '@gaming-cafe/ui';
import { Visibility } from '@mui/icons-material';
import { MenuItem, TextField } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  getPurchaseOrders,
  type PurchaseOrder,
  type PurchaseOrderStatus,
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

export default function PurchaseOrdersPage() {
  const navigate = useNavigate();
  const [status, setStatus] = useState('');
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const query = useQuery({
    queryKey: ['purchase-orders', status, search, page],
    queryFn: () =>
      getPurchaseOrders({
        status: status || undefined,
        search: search || undefined,
        page,
        limit: 25,
      }),
  });
  const columns: Column<PurchaseOrder>[] = [
    { id: 'poNumber', label: 'PO number', minWidth: 150 },
    {
      id: 'status',
      label: 'Status',
      minWidth: 150,
      format: (value) => (
        <StatusBadge label={String(value)} tone={tones[value as PurchaseOrderStatus]} />
      ),
    },
    {
      id: 'expectedDeliveryDate',
      label: 'Expected',
      minWidth: 120,
      format: (value) => (value ? String(value) : '—'),
    },
    {
      id: 'total',
      label: 'Total',
      align: 'right',
      format: (value) => currency.format(Number(value)),
    },
    {
      id: 'updatedAt',
      label: 'Updated',
      minWidth: 170,
      format: (value) => formatDisplayDateTime(String(value)),
    },
  ];
  return (
    <ListPage
      title="Purchase orders"
      description="Draft, approve, order, and receive stock with a complete audit trail."
      columns={columns}
      data={query.data?.data ?? []}
      isLoading={query.isLoading}
      showSearch
      searchValue={search}
      onSearchChange={(event) => {
        setSearch(event.target.value);
        setPage(1);
      }}
      onSearchClear={() => setSearch('')}
      onAddClick={() => navigate('/inventory/purchase-orders/new')}
      addButtonLabel="New purchase order"
      filters={
        <TextField
          select
          size="small"
          label="Status"
          value={status}
          onChange={(event) => {
            setStatus(event.target.value);
            setPage(1);
          }}
          sx={{ minWidth: 200 }}
        >
          <MenuItem value="">All statuses</MenuItem>
          {Object.keys(tones).map((value) => (
            <MenuItem key={value} value={value}>
              {value.replaceAll('_', ' ')}
            </MenuItem>
          ))}
        </TextField>
      }
      actions={[
        {
          icon: <Visibility fontSize="small" />,
          label: 'Open',
          onClick: (row) => navigate(`/inventory/purchase-orders/${row.id}`),
        },
      ]}
      pagination={{ page, totalPages: query.data?.totalPages ?? 1, onPageChange: setPage }}
      emptyMessage="No purchase orders"
      emptyDescription="Create a draft purchase order to begin procurement."
    />
  );
}
