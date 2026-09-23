import { type Column, ListPage, StatusBadge } from '@gaming-cafe/ui';
import { Autocomplete, MenuItem, TextField } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import {
  getInventoryLocations,
  getStockMovements,
  type StockMovementRow,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { formatDisplayDateTime } from '../../../utils/date';

export default function InventoryMovementsPage() {
  const [locationId, setLocationId] = useState('');
  const [movementType, setMovementType] = useState('');
  const [product, setProduct] = useState<ProductResponse | null>(null);
  const [referenceType, setReferenceType] = useState('');
  const [fromDate, setFromDate] = useState('');
  const [toDate, setToDate] = useState('');
  const [page, setPage] = useState(1);
  const locations = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 100 }),
  });
  const products = useQuery({
    queryKey: ['products-movement-filter'],
    queryFn: () => getProducts({ limit: 500, sortBy: 'name', sortOrder: 'ASC' }),
  });
  const movements = useQuery({
    queryKey: [
      'inventory-movements',
      locationId,
      product?.id,
      movementType,
      referenceType,
      fromDate,
      toDate,
      page,
    ],
    queryFn: () =>
      getStockMovements({
        locationId: locationId || undefined,
        productId: product?.id,
        movementType: movementType || undefined,
        referenceType: referenceType || undefined,
        from: fromDate ? new Date(`${fromDate}T00:00:00`).toISOString() : undefined,
        to: toDate ? new Date(`${toDate}T23:59:59.999`).toISOString() : undefined,
        page,
        limit: 30,
      }),
  });
  const columns: Column<StockMovementRow>[] = [
    {
      id: 'createdAt',
      label: 'When',
      minWidth: 170,
      format: (value) => formatDisplayDateTime(value as string),
    },
    { id: 'productName', label: 'Product', minWidth: 180 },
    { id: 'locationName', label: 'Location', minWidth: 160 },
    {
      id: 'movementType',
      label: 'Type',
      minWidth: 130,
      format: (value) => <StatusBadge label={String(value)} tone="info" />,
    },
    {
      id: 'delta',
      label: 'Change',
      align: 'right',
      format: (value) => (
        <StatusBadge
          label={`${Number(value) > 0 ? '+' : ''}${value}`}
          tone={Number(value) >= 0 ? 'success' : 'warning'}
        />
      ),
    },
    {
      id: 'referenceType',
      label: 'Reference',
      minWidth: 150,
      format: (value) => (value ? String(value).replaceAll('_', ' ') : '—'),
    },
  ];
  return (
    <ListPage
      title="Stock movement ledger"
      description="Immutable stock changes across every warehouse and store."
      columns={columns}
      data={movements.data?.data ?? []}
      isLoading={movements.isLoading}
      pagination={{ page, totalPages: movements.data?.totalPages ?? 1, onPageChange: setPage }}
      filters={
        <>
          <TextField
            select
            size="small"
            label="Location"
            value={locationId}
            onChange={(event) => {
              setLocationId(event.target.value);
              setPage(1);
            }}
            sx={{ minWidth: 190 }}
          >
            <MenuItem value="">All locations</MenuItem>
            {locations.data?.data.map((location) => (
              <MenuItem key={location.id} value={location.id}>
                {location.name}
              </MenuItem>
            ))}
          </TextField>
          <Autocomplete
            size="small"
            options={products.data?.data ?? []}
            value={product}
            getOptionLabel={(item) => item.name}
            onChange={(_, next) => {
              setProduct(next);
              setPage(1);
            }}
            renderInput={(params) => <TextField {...params} label="Product" />}
            sx={{ minWidth: 210 }}
          />
          <TextField
            select
            size="small"
            label="Movement type"
            value={movementType}
            onChange={(event) => {
              setMovementType(event.target.value);
              setPage(1);
            }}
            sx={{ minWidth: 180 }}
          >
            <MenuItem value="">All types</MenuItem>
            {[
              'receipt',
              'adjustment',
              'transfer_out',
              'transfer_in',
              'waste',
              'sale',
              'refund',
            ].map((type) => (
              <MenuItem key={type} value={type}>
                {type.replaceAll('_', ' ')}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            size="small"
            label="Reference type"
            value={referenceType}
            onChange={(event) => {
              setReferenceType(event.target.value);
              setPage(1);
            }}
            sx={{ minWidth: 170 }}
          />
          <TextField
            size="small"
            type="date"
            label="From"
            value={fromDate}
            onChange={(event) => {
              setFromDate(event.target.value);
              setPage(1);
            }}
            InputLabelProps={{ shrink: true }}
          />
          <TextField
            size="small"
            type="date"
            label="To"
            value={toDate}
            onChange={(event) => {
              setToDate(event.target.value);
              setPage(1);
            }}
            InputLabelProps={{ shrink: true }}
          />
        </>
      }
    />
  );
}
