import { type Column, IntegerField, ListPage, StatusBadge } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { EditNote } from '@mui/icons-material';
import {
  Autocomplete,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  createStockAdjustment,
  getInventoryLocations,
  getLocationStock,
  type LocationStockRow,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';

type StockDisplayRow = LocationStockRow & { id: string };

export default function InventoryStockPage() {
  const { can } = usePermissions();
  const canManage = can(Permission.InventoryManage);
  const client = useQueryClient();
  const [locationId, setLocationId] = useState('');
  const [search, setSearch] = useState('');
  const [lowStock, setLowStock] = useState(false);
  const [page, setPage] = useState(1);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [product, setProduct] = useState<ProductResponse | null>(null);
  const [counted, setCounted] = useState('');
  const [notes, setNotes] = useState('');
  const locations = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 100 }),
  });
  const stock = useQuery({
    queryKey: ['location-stock', locationId, search, lowStock, page],
    queryFn: () =>
      getLocationStock({
        locationId: locationId || undefined,
        search: search || undefined,
        lowStock: lowStock || undefined,
        page,
        limit: 30,
        sortBy: 'product',
        sortOrder: 'asc',
      }),
  });
  const products = useQuery({
    queryKey: ['products-stock'],
    queryFn: () => getProducts({ limit: 500, sortBy: 'name', sortOrder: 'ASC' }),
  });
  const selectedRow = product
    ? stock.data?.data.find(
        (row) => row.productId === product.id && (!locationId || row.locationId === locationId),
      )
    : undefined;
  const reconcile = useMutation({
    mutationFn: () =>
      createStockAdjustment({
        locationId,
        notes: notes.trim(),
        lines: [{ productId: product?.id ?? '', countedPieces: Number(counted) }],
      }),
    onSuccess: () => {
      toastUtils.success('Stock count reconciled');
      setDialogOpen(false);
      void client.invalidateQueries({ queryKey: ['location-stock'] });
      void client.invalidateQueries({ queryKey: ['inventory-overview'] });
    },
    onError: (error: unknown) =>
      toastUtils.error(error instanceof Error ? error.message : 'Reconciliation failed'),
  });
  const openReconcile = (row?: StockDisplayRow) => {
    const nextLocation = row?.locationId ?? locationId;
    setLocationId(nextLocation);
    const nextProduct = row
      ? (products.data?.data.find((item) => item.id === row.productId) ?? null)
      : null;
    setProduct(nextProduct);
    setCounted(row ? String(row.quantityPieces) : '');
    setNotes('');
    setDialogOpen(true);
  };
  const displayRows: StockDisplayRow[] = (stock.data?.data ?? []).map((row) => ({
    ...row,
    id: `${row.locationId}:${row.productId}`,
  }));
  const columns: Column<StockDisplayRow>[] = [
    { id: 'productName', label: 'Product', minWidth: 190 },
    {
      id: 'productSku',
      label: 'SKU',
      minWidth: 110,
      format: (value) => (value ? String(value) : '—'),
    },
    { id: 'locationName', label: 'Location', minWidth: 170 },
    {
      id: 'quantityPieces',
      label: 'On hand',
      align: 'right',
      format: (value) => (
        <Typography fontWeight={700}>{Number(value).toLocaleString('en-IN')} pcs</Typography>
      ),
    },
  ];
  return (
    <>
      <ListPage
        title="Stock by location"
        description="Each warehouse and store is shown as a first-class location; no quantities are silently aggregated."
        columns={columns}
        data={displayRows}
        isLoading={stock.isLoading}
        showSearch
        searchValue={search}
        onSearchChange={(event) => {
          setSearch(event.target.value);
          setPage(1);
        }}
        onSearchClear={() => setSearch('')}
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
              sx={{ minWidth: 210 }}
            >
              <MenuItem value="">All locations</MenuItem>
              {locations.data?.data
                .filter((item) => item.isActive)
                .map((item) => (
                  <MenuItem key={item.id} value={item.id}>
                    {item.name} ({item.kind})
                  </MenuItem>
                ))}
            </TextField>
            <Button
              variant={lowStock ? 'contained' : 'outlined'}
              onClick={() => {
                setLowStock((value) => !value);
                setPage(1);
              }}
            >
              {lowStock ? 'Showing low stock' : 'Low stock only'}
            </Button>
          </>
        }
        onAddClick={canManage && locationId ? () => openReconcile() : undefined}
        addButtonLabel="Reconcile count"
        actions={
          canManage
            ? [{ icon: <EditNote fontSize="small" />, label: 'Reconcile', onClick: openReconcile }]
            : []
        }
        pagination={{ page, totalPages: stock.data?.totalPages ?? 1, onPageChange: setPage }}
        emptyMessage="No stock matches these filters"
        emptyDescription="Try another location or clear the low-stock filter."
      />
      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Reconcile physical count</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
            <TextField
              select
              label="Location"
              value={locationId}
              onChange={(event) => setLocationId(event.target.value)}
              required
            >
              {locations.data?.data
                .filter((item) => item.isActive)
                .map((item) => (
                  <MenuItem key={item.id} value={item.id}>
                    {item.name}
                  </MenuItem>
                ))}
            </TextField>
            <Autocomplete
              options={products.data?.data ?? []}
              value={product}
              getOptionLabel={(item) => item.name}
              onChange={(_, item) => setProduct(item)}
              renderInput={(params) => <TextField {...params} label="Product" required />}
            />
            {selectedRow ? (
              <StatusBadge
                label={`System count ${selectedRow.quantityPieces} pieces`}
                tone="info"
              />
            ) : null}
            <IntegerField
              label="Counted pieces"
              value={counted}
              onChange={(event) => setCounted(event.target.value)}
              inputProps={{ min: 0 }}
              required
            />
            <TextField
              label="Reason / audit note"
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
              multiline
              rows={3}
              required
              helperText="Required for the immutable stock movement audit trail."
            />
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDialogOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            disabled={
              !locationId ||
              !product ||
              counted === '' ||
              notes.trim().length < 3 ||
              reconcile.isPending
            }
            onClick={() => reconcile.mutate()}
          >
            Apply reconciliation
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
