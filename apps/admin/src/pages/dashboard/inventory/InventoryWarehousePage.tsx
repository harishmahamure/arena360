import { type Column, IntegerField, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Card,
  CardContent,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getVendors } from '../../../services/expenses';
import {
  createStockReceipt,
  getInventoryLocations,
  getLocationStock,
} from '../../../services/inventory';
import { getProducts } from '../../../services/product/list';

type StockRow = {
  id: string;
  productName?: string | null;
  productSku?: string | null;
  quantityPieces: number;
};

export default function InventoryWarehousePage() {
  const { can } = usePermissions();
  const canManage = can(Permission.InventoryManage);
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const warehouse = useMemo(
    () => locations?.data.find((l) => l.kind === 'warehouse' && l.isActive),
    [locations],
  );

  const { data: stock, isLoading } = useQuery({
    queryKey: ['warehouse-stock', warehouse?.id],
    queryFn: () => getLocationStock({ locationId: warehouse?.id, limit: 100, page: 1 }),
    enabled: !!warehouse?.id,
  });

  const { data: productsData } = useQuery({
    queryKey: ['products-for-receive'],
    queryFn: () => getProducts({ limit: 200, sortBy: 'name', sortOrder: 'ASC' }),
    enabled: canManage,
  });

  const { data: vendorsData } = useQuery({
    queryKey: ['vendors-for-receive'],
    queryFn: () => getVendors({ limit: 100 }),
    enabled: canManage,
  });

  const [productId, setProductId] = useState('');
  const [boxQty, setBoxQty] = useState('1');
  const [vendorId, setVendorId] = useState('');
  const [notes, setNotes] = useState('');

  const selectedProduct = productsData?.data.find((p) => p.id === productId);
  const unitsPerBox =
    (selectedProduct as { unitsPerPurchaseUnit?: number })?.unitsPerPurchaseUnit ?? 1;
  const piecesPreview = Number(boxQty || 0) * unitsPerBox;

  const receiveMutation = useMutation({
    mutationFn: async () => {
      if (!warehouse) throw new Error('No warehouse configured');
      return createStockReceipt({
        locationId: warehouse.id,
        vendorId: vendorId || undefined,
        notes: notes || undefined,
        lines: [{ productId, boxQuantity: Number(boxQty) }],
      });
    },
    onSuccess: () => {
      toastUtils.success(`Received ${piecesPreview} pieces`);
      queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      setProductId('');
      setBoxQty('1');
      setNotes('');
    },
    onError: () => toastUtils.error('Failed to receive stock'),
  });

  const rows: StockRow[] = useMemo(() => {
    const list = stock?.data ?? [];
    return list
      .map((r) => ({
        id: `${r.locationId}-${r.productId}`,
        productName: r.productName,
        productSku: r.productSku,
        quantityPieces: r.quantityPieces,
      }))
      .filter((r) => (r.productName ?? '').toLowerCase().includes(search.toLowerCase()));
  }, [stock, search]);

  const columns: Column<StockRow>[] = [
    { id: 'productName', label: 'Product', minWidth: 200, format: (v) => (v as string) || '—' },
    { id: 'productSku', label: 'SKU', minWidth: 100, format: (v) => (v as string) || '—' },
    {
      id: 'quantityPieces',
      label: 'Pieces in stock',
      minWidth: 120,
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
  ];

  if (!warehouse) {
    return (
      <Alert severity="warning" sx={{ m: 4 }}>
        No active warehouse location found. Add one under Inventory → Locations.
      </Alert>
    );
  }

  return (
    <>
      <ListPage
        title={`Warehouse Stock — ${warehouse.name}`}
        description="Stock levels at the warehouse (pieces)"
        columns={columns}
        data={rows}
        actions={[]}
        isLoading={isLoading}
        showSearch
        searchValue={search}
        onSearchChange={(e) => setSearch(e.target.value)}
        onSearchClear={() => setSearch('')}
      />

      {canManage && (
        <Paper sx={{ p: 3, mt: 3, mx: { xs: 2, md: 4 } }}>
          <Typography variant="h6" fontWeight={600} gutterBottom>
            Receive Stock
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Enter quantity in boxes; pieces are calculated from the product&apos;s units per box.
          </Typography>
          <Card variant="outlined">
            <CardContent>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: '1fr', md: '1fr 1fr' },
                  gap: 2,
                }}
              >
                <Autocomplete
                  options={productsData?.data ?? []}
                  getOptionLabel={(p) => p.name}
                  value={productsData?.data.find((p) => p.id === productId) ?? null}
                  onChange={(_, v) => setProductId(v?.id ?? '')}
                  renderInput={(params) => <TextField {...params} label="Product" required />}
                />
                <TextField
                  select
                  label="Vendor (optional)"
                  value={vendorId}
                  onChange={(e) => setVendorId(e.target.value)}
                >
                  <MenuItem value="">None</MenuItem>
                  {(vendorsData?.data ?? []).map((v) => (
                    <MenuItem key={v.id} value={v.id}>
                      {v.name}
                    </MenuItem>
                  ))}
                </TextField>
                <IntegerField
                  label="Boxes received"
                  value={boxQty}
                  onChange={(e) => setBoxQty(e.target.value)}
                  inputProps={{ min: 1 }}
                  required
                />
                <TextField
                  label="Notes"
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  fullWidth
                />
              </Box>
              {productId && (
                <Typography variant="body2" sx={{ mt: 2 }} color="text.secondary">
                  {boxQty || 0} box(es) × {unitsPerBox} pcs/box ={' '}
                  <strong>{piecesPreview} pieces</strong>
                </Typography>
              )}
              <Button
                variant="contained"
                sx={{ mt: 2 }}
                disabled={!productId || !boxQty || receiveMutation.isPending}
                onClick={() => receiveMutation.mutate()}
              >
                Receive into warehouse
              </Button>
            </CardContent>
          </Card>
        </Paper>
      )}
    </>
  );
}
