import { type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { EditNote } from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  createStockAdjustment,
  getInventoryLocations,
  getLocationStock,
  getStockAdjustments,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { formatDisplayDateTime } from '../../../utils/date';

interface CombinedStockRow {
  id: string;
  productId: string;
  productName: string;
  productSku?: string | null;
  warehousePieces: number;
  storePieces: number;
}

export default function InventoryStockPage() {
  const { can } = usePermissions();
  const canManage = can(Permission.InventoryManage);
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');

  const [reconcileOpen, setReconcileOpen] = useState(false);
  const [reconcileLocationId, setReconcileLocationId] = useState('');
  const [reconcileProduct, setReconcileProduct] = useState<ProductResponse | null>(null);
  const [countedPieces, setCountedPieces] = useState('');
  const [reconcileNotes, setReconcileNotes] = useState('');

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const warehouse = useMemo(
    () => locations?.data.find((l) => l.kind === 'warehouse' && l.isActive),
    [locations],
  );
  const stores = useMemo(
    () => locations?.data.filter((l) => l.kind === 'store' && l.isActive) ?? [],
    [locations],
  );
  const storeLabel =
    stores.length === 1 ? stores[0]?.name : stores.length > 1 ? 'All stores' : 'Store';

  const { data: warehouseStock, isLoading: warehouseLoading } = useQuery({
    queryKey: ['stock-overview-warehouse', warehouse?.id],
    queryFn: () => getLocationStock({ locationId: warehouse?.id, limit: 500, page: 1 }),
    enabled: !!warehouse?.id,
  });

  const storeStockQueries = useQueries({
    queries: stores.map((s) => ({
      queryKey: ['stock-overview-store', s.id],
      queryFn: () => getLocationStock({ locationId: s.id, limit: 500, page: 1 }),
      enabled: !!s.id,
    })),
  });

  const storeStockByProduct = useMemo(() => {
    const map = new Map<string, number>();
    for (const q of storeStockQueries) {
      for (const row of q.data?.data ?? []) {
        map.set(row.productId, (map.get(row.productId) ?? 0) + row.quantityPieces);
      }
    }
    return map;
  }, [storeStockQueries]);

  const storeLoading = storeStockQueries.some((q) => q.isLoading);

  const { data: productsData } = useQuery({
    queryKey: ['products-stock-overview'],
    queryFn: () => getProducts({ limit: 500, sortBy: 'name', sortOrder: 'ASC' }),
  });

  const { data: adjustments } = useQuery({
    queryKey: ['stock-adjustments'],
    queryFn: () => getStockAdjustments({ limit: 10, page: 1 }),
  });

  const locationName = (id: string) =>
    locations?.data.find((l) => l.id === id)?.name ?? id.slice(0, 8);

  const rows: CombinedStockRow[] = useMemo(() => {
    const map = new Map<string, CombinedStockRow>();

    for (const row of warehouseStock?.data ?? []) {
      map.set(row.productId, {
        id: row.productId,
        productId: row.productId,
        productName: row.productName ?? 'Unknown',
        productSku: row.productSku,
        warehousePieces: row.quantityPieces,
        storePieces: 0,
      });
    }

    for (const [productId, qty] of storeStockByProduct) {
      const existing = map.get(productId);
      const sampleRow = storeStockQueries
        .flatMap((q) => q.data?.data ?? [])
        .find((r) => r.productId === productId);
      if (existing) {
        existing.storePieces = qty;
      } else {
        map.set(productId, {
          id: productId,
          productId,
          productName: sampleRow?.productName ?? 'Unknown',
          productSku: sampleRow?.productSku,
          warehousePieces: 0,
          storePieces: qty,
        });
      }
    }

    return [...map.values()]
      .filter((r) => r.productName.toLowerCase().includes(search.toLowerCase()))
      .sort((a, b) => a.productName.localeCompare(b.productName));
  }, [warehouseStock, storeStockByProduct, storeStockQueries, search]);

  const stockAtLocation = (locationId: string, productId: string) => {
    if (locationId === warehouse?.id) {
      return warehouseStock?.data.find((r) => r.productId === productId)?.quantityPieces ?? 0;
    }
    const storeIdx = stores.findIndex((s) => s.id === locationId);
    if (storeIdx >= 0) {
      return (
        storeStockQueries[storeIdx]?.data?.data.find((r) => r.productId === productId)
          ?.quantityPieces ?? 0
      );
    }
    return 0;
  };

  const openReconcile = (product?: CombinedStockRow, locationId?: string) => {
    const loc = locationId ?? warehouse?.id ?? stores[0]?.id ?? '';
    setReconcileLocationId(loc);
    if (product) {
      const full = productsData?.data.find((p) => p.id === product.productId) ?? null;
      setReconcileProduct(full);
      setCountedPieces(String(stockAtLocation(loc, product.productId)));
    } else {
      setReconcileProduct(null);
      setCountedPieces('');
    }
    setReconcileNotes('');
    setReconcileOpen(true);
  };

  const reconcileMut = useMutation({
    mutationFn: () => {
      if (!reconcileProduct || !reconcileLocationId) {
        throw new Error('Select location and product');
      }
      return createStockAdjustment({
        locationId: reconcileLocationId,
        notes: reconcileNotes.trim(),
        lines: [
          {
            productId: reconcileProduct.id,
            countedPieces: Number(countedPieces),
          },
        ],
      });
    },
    onSuccess: () => {
      toastUtils.success('Stock reconciled');
      setReconcileOpen(false);
      void queryClient.invalidateQueries({ queryKey: ['stock-overview-warehouse'] });
      void queryClient.invalidateQueries({ queryKey: ['stock-overview-store'] });
      void queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      void queryClient.invalidateQueries({ queryKey: ['stock-adjustments'] });
    },
    onError: (err: unknown) => {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to reconcile stock');
    },
  });

  const columns: Column<CombinedStockRow>[] = [
    { id: 'productName', label: 'Product', minWidth: 180 },
    {
      id: 'productSku',
      label: 'SKU',
      minWidth: 100,
      format: (v) => (v as string) || '—',
    },
    {
      id: 'warehousePieces',
      label: warehouse?.name ? `${warehouse.name} (pcs)` : 'Warehouse (pcs)',
      minWidth: 120,
      align: 'right',
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
    {
      id: 'storePieces',
      label: storeLabel ? `${storeLabel} (pcs)` : 'Store (pcs)',
      minWidth: 120,
      align: 'right',
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
    {
      id: 'id',
      key: 'total',
      label: 'Total (pcs)',
      minWidth: 100,
      align: 'right',
      format: (_, row) => row.warehousePieces + row.storePieces,
    },
  ];

  const actions = canManage
    ? [
        {
          icon: <EditNote fontSize="small" />,
          label: 'Reconcile',
          onClick: (row: CombinedStockRow) => openReconcile(row),
        },
      ]
    : [];

  if (!warehouse && stores.length === 0) {
    return (
      <Alert severity="warning" sx={{ m: 4 }}>
        No active warehouse or store locations found. Configure them under Inventory → Locations.
      </Alert>
    );
  }

  const systemCurrent =
    reconcileProduct && reconcileLocationId
      ? stockAtLocation(reconcileLocationId, reconcileProduct.id)
      : 0;

  return (
    <>
      <ListPage
        title="Stock overview"
        description="Warehouse and store quantities by product. Reconcile after physical counts."
        columns={columns}
        data={rows}
        actions={actions}
        isLoading={warehouseLoading || storeLoading}
        showSearch
        searchValue={search}
        onSearchChange={(e) => setSearch(e.target.value)}
        onSearchClear={() => setSearch('')}
        onAddClick={canManage ? () => openReconcile() : undefined}
        addButtonLabel="Reconcile stock"
      />

      {adjustments?.data.length ? (
        <Paper sx={{ p: 3, mt: 3, mx: { xs: 2, md: 4 } }}>
          <Typography variant="h6" fontWeight={600} gutterBottom>
            Recent reconciliations
          </Typography>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>When</TableCell>
                <TableCell>Location</TableCell>
                <TableCell>Notes</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {adjustments.data.map((adj) => (
                <TableRow key={adj.id}>
                  <TableCell>{formatDisplayDateTime(adj.createdAt)}</TableCell>
                  <TableCell>{locationName(adj.locationId)}</TableCell>
                  <TableCell>{adj.notes}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Paper>
      ) : null}

      <Dialog open={reconcileOpen} onClose={() => setReconcileOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>Reconcile stock count</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
            <TextField
              select
              label="Location"
              value={reconcileLocationId}
              onChange={(e) => {
                setReconcileLocationId(e.target.value);
                if (reconcileProduct) {
                  setCountedPieces(String(stockAtLocation(e.target.value, reconcileProduct.id)));
                }
              }}
              required
            >
              {locations?.data
                .filter((l) => l.isActive)
                .map((l) => (
                  <MenuItem key={l.id} value={l.id}>
                    {l.name} ({l.kind})
                  </MenuItem>
                ))}
            </TextField>
            <Autocomplete
              options={productsData?.data ?? []}
              getOptionLabel={(p) => p.name}
              value={reconcileProduct}
              onChange={(_, p) => {
                setReconcileProduct(p);
                if (p && reconcileLocationId) {
                  setCountedPieces(String(stockAtLocation(reconcileLocationId, p.id)));
                }
              }}
              renderInput={(params) => <TextField {...params} label="Product" required />}
            />
            {reconcileProduct ? (
              <Typography variant="body2" color="text.secondary">
                System quantity: <strong>{systemCurrent} pieces</strong>
              </Typography>
            ) : null}
            <TextField
              type="number"
              label="Counted pieces"
              value={countedPieces}
              onChange={(e) => setCountedPieces(e.target.value)}
              inputProps={{ min: 0 }}
              required
            />
            <TextField
              label="Comments"
              value={reconcileNotes}
              onChange={(e) => setReconcileNotes(e.target.value)}
              multiline
              rows={3}
              required
              helperText="Why this count differs (e.g. physical audit, damaged units removed)"
            />
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setReconcileOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            disabled={
              !reconcileProduct ||
              !reconcileLocationId ||
              reconcileNotes.trim().length < 3 ||
              countedPieces === '' ||
              reconcileMut.isPending
            }
            onClick={() => reconcileMut.mutate()}
          >
            Apply reconciliation
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
