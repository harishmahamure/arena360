import { type Column, IntegerField, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Delete } from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Card,
  CardContent,
  IconButton,
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
import { getProducts, type ProductResponse } from '../../../services/product/list';

type StockRow = {
  id: string;
  productName?: string | null;
  productSku?: string | null;
  quantityPieces: number;
};

interface ReceiveLine {
  productId: string;
  productName: string;
  boxQuantity: number;
  unitsPerBox: number;
}

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

  const [lines, setLines] = useState<ReceiveLine[]>([]);
  const [productInput, setProductInput] = useState('');
  const [boxQty, setBoxQty] = useState('1');
  const [vendorId, setVendorId] = useState('');
  const [notes, setNotes] = useState('');

  const productOptions = useMemo(() => {
    if (productInput.length < 1) return productsData?.data.slice(0, 20) ?? [];
    return (productsData?.data ?? []).filter((p) =>
      p.name.toLowerCase().includes(productInput.toLowerCase()),
    );
  }, [productsData, productInput]);

  const totalBoxes = lines.reduce((sum, l) => sum + l.boxQuantity, 0);
  const totalPieces = lines.reduce((sum, l) => sum + l.boxQuantity * l.unitsPerBox, 0);

  const addLine = (product: ProductResponse) => {
    const unitsPerBox = product.unitsPerPurchaseUnit ?? 1;
    const qty = Math.max(1, Number(boxQty) || 1);
    setLines((prev) => {
      const existing = prev.find((l) => l.productId === product.id);
      if (existing) {
        return prev.map((l) =>
          l.productId === product.id ? { ...l, boxQuantity: l.boxQuantity + qty, unitsPerBox } : l,
        );
      }
      return [
        ...prev,
        {
          productId: product.id,
          productName: product.name,
          boxQuantity: qty,
          unitsPerBox,
        },
      ];
    });
    setProductInput('');
    setBoxQty('1');
  };

  const receiveMutation = useMutation({
    mutationFn: async () => {
      if (!warehouse) throw new Error('No warehouse configured');
      return createStockReceipt({
        locationId: warehouse.id,
        vendorId: vendorId || undefined,
        notes: notes || undefined,
        lines: lines.map((l) => ({
          productId: l.productId,
          boxQuantity: l.boxQuantity,
        })),
      });
    },
    onSuccess: () => {
      toastUtils.success(
        `Received ${totalBoxes} box(es) · ${totalPieces} pieces across ${lines.length} product(s)`,
      );
      queryClient.invalidateQueries({ queryKey: ['warehouse-stock'] });
      setLines([]);
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
            Add multiple products from one vendor delivery. Enter quantity in boxes; pieces are
            calculated from each product&apos;s units per box.
          </Typography>
          <Card variant="outlined">
            <CardContent>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: '1fr', md: '1fr 1fr' },
                  gap: 2,
                  mb: 2,
                }}
              >
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
                <TextField
                  label="Notes"
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  fullWidth
                />
              </Box>

              <Box
                sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', mb: 2, alignItems: 'flex-start' }}
              >
                <Autocomplete
                  sx={{ flex: 1, minWidth: 240 }}
                  options={productOptions}
                  getOptionLabel={(p) => p.name}
                  inputValue={productInput}
                  onInputChange={(_, v) => setProductInput(v)}
                  onChange={(_, p) => p && addLine(p)}
                  renderInput={(params) => (
                    <TextField
                      {...params}
                      label="Add product"
                      placeholder="Search product..."
                      helperText="Select a product to add a line"
                    />
                  )}
                />
                <IntegerField
                  label="Boxes"
                  value={boxQty}
                  onChange={(e) => setBoxQty(e.target.value)}
                  inputProps={{ min: 1 }}
                  sx={{ width: 120 }}
                  helperText="Qty per add"
                />
              </Box>

              {lines.length === 0 ? (
                <Typography color="text.secondary" sx={{ mb: 2 }}>
                  No products added yet. Search above to build the receipt.
                </Typography>
              ) : (
                lines.map((line) => (
                  <Box
                    key={line.productId}
                    sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 1 }}
                  >
                    <Box sx={{ flex: 1 }}>
                      <Typography>{line.productName}</Typography>
                      <Typography variant="caption" color="text.secondary">
                        {line.boxQuantity} box(es) × {line.unitsPerBox} pcs ={' '}
                        {line.boxQuantity * line.unitsPerBox} pieces
                      </Typography>
                    </Box>
                    <IntegerField
                      size="small"
                      label="Boxes"
                      value={line.boxQuantity}
                      onChange={(e) => {
                        const v = Math.max(1, Number(e.target.value) || 1);
                        setLines((prev) =>
                          prev.map((l) =>
                            l.productId === line.productId ? { ...l, boxQuantity: v } : l,
                          ),
                        );
                      }}
                      inputProps={{ min: 1 }}
                      sx={{ width: 100 }}
                    />
                    <IconButton
                      color="error"
                      onClick={() =>
                        setLines((prev) => prev.filter((l) => l.productId !== line.productId))
                      }
                    >
                      <Delete />
                    </IconButton>
                  </Box>
                ))
              )}

              {lines.length > 0 && (
                <Typography variant="body2" sx={{ mt: 2 }} color="text.secondary">
                  Total: <strong>{totalBoxes}</strong> box(es) · <strong>{totalPieces}</strong>{' '}
                  pieces · <strong>{lines.length}</strong> product(s)
                </Typography>
              )}

              <Button
                variant="contained"
                sx={{ mt: 2 }}
                disabled={lines.length === 0 || receiveMutation.isPending}
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
