import { FormPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Delete } from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  IconButton,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { createTransferRequest, getInventoryLocations } from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { formatTransferQuantity, piecesFromQuantityInput } from './transferQuantity';

type QuantityMode = 'pieces' | 'boxes';

interface LineRow {
  productId: string;
  productName: string;
  unitsPerBox: number;
  quantityPieces: number;
}

export default function InventoryTransferNewPage() {
  const navigate = useNavigate();
  const [lines, setLines] = useState<LineRow[]>([]);
  const [productInput, setProductInput] = useState('');
  const [qty, setQty] = useState('1');
  const [quantityMode, setQuantityMode] = useState<QuantityMode>('boxes');

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const store = useMemo(
    () => locations?.data.find((l) => l.kind === 'store' && l.isActive),
    [locations],
  );
  const warehouse = useMemo(
    () => locations?.data.find((l) => l.kind === 'warehouse' && l.isActive),
    [locations],
  );

  const { data: productsData } = useQuery({
    queryKey: ['products-transfer'],
    queryFn: () => getProducts({ limit: 200 }),
  });

  const productOptions = useMemo(() => {
    if (productInput.length < 1) return productsData?.data.slice(0, 20) ?? [];
    return (productsData?.data ?? []).filter((p) =>
      p.name.toLowerCase().includes(productInput.toLowerCase()),
    );
  }, [productsData, productInput]);

  const addLine = (product: ProductResponse) => {
    const unitsPerBox = product.unitsPerPurchaseUnit ?? 1;
    const pieces = piecesFromQuantityInput(Number(qty) || 1, quantityMode, unitsPerBox);
    setLines((prev) => {
      const existing = prev.find((l) => l.productId === product.id);
      if (existing) {
        return prev.map((l) =>
          l.productId === product.id
            ? { ...l, quantityPieces: l.quantityPieces + pieces, unitsPerBox }
            : l,
        );
      }
      return [
        ...prev,
        {
          productId: product.id,
          productName: product.name,
          unitsPerBox,
          quantityPieces: pieces,
        },
      ];
    });
    setProductInput('');
    setQty('1');
  };

  const createMut = useMutation({
    mutationFn: () =>
      createTransferRequest({
        fromLocationId: warehouse?.id,
        toLocationId: store?.id,
        lines: lines.map((l) => ({
          productId: l.productId,
          quantityPieces: l.quantityPieces,
        })),
      }),
    onSuccess: () => {
      toastUtils.success('Transfer request submitted');
      navigate('/inventory/transfers');
    },
    onError: () => toastUtils.error('Failed to create request'),
  });

  if (!store || !warehouse) {
    return (
      <FormPage
        title="Request stock for store"
        description="Transfer stock from warehouse to store"
        backTo="/inventory/transfers"
        backLabel="Back to transfers"
        breadcrumbs={[
          { label: 'Inventory', to: '/inventory/transfers' },
          { label: 'New transfer' },
        ]}
      >
        <Alert severity="warning">Configure active warehouse and store locations first.</Alert>
      </FormPage>
    );
  }

  return (
    <FormPage
      title="Request stock for store"
      description={`Request: ${warehouse.name} → ${store.name}`}
      backTo="/inventory/transfers"
      backLabel="Back to transfers"
      breadcrumbs={[{ label: 'Inventory', to: '/inventory/transfers' }, { label: 'New transfer' }]}
    >
      <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', mb: 3, alignItems: 'flex-start' }}>
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
              helperText="Search and select a product to add to the transfer request"
            />
          )}
        />
        <ToggleButtonGroup
          exclusive
          size="small"
          value={quantityMode}
          onChange={(_, value: QuantityMode | null) => value && setQuantityMode(value)}
          sx={{ alignSelf: 'center' }}
        >
          <ToggleButton value="boxes">Boxes</ToggleButton>
          <ToggleButton value="pieces">Pieces</ToggleButton>
        </ToggleButtonGroup>
        <TextField
          type="number"
          label={quantityMode === 'boxes' ? 'Boxes' : 'Pieces'}
          value={qty}
          onChange={(e) => setQty(e.target.value)}
          inputProps={{ min: 1 }}
          sx={{ width: 120 }}
          helperText="Qty per add"
        />
      </Box>

      {lines.length === 0 ? (
        <Typography color="text.secondary">
          No items added yet. Search for products above to build the request.
        </Typography>
      ) : (
        lines.map((line) => (
          <Box key={line.productId} sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 1 }}>
            <Box sx={{ flex: 1 }}>
              <Typography>{line.productName}</Typography>
              <Typography variant="caption" color="text.secondary">
                {formatTransferQuantity(line.quantityPieces, line.unitsPerBox)}
              </Typography>
            </Box>
            <TextField
              type="number"
              size="small"
              label="Pieces"
              value={line.quantityPieces}
              onChange={(e) => {
                const v = Math.max(1, Number(e.target.value) || 1);
                setLines((prev) =>
                  prev.map((l) =>
                    l.productId === line.productId ? { ...l, quantityPieces: v } : l,
                  ),
                );
              }}
              inputProps={{ min: 1 }}
              sx={{ width: 100 }}
            />
            <IconButton
              color="error"
              onClick={() => setLines((prev) => prev.filter((l) => l.productId !== line.productId))}
            >
              <Delete />
            </IconButton>
          </Box>
        ))
      )}

      <Box sx={{ mt: 3, display: 'flex', gap: 2 }}>
        <Button variant="outlined" onClick={() => navigate('/inventory/transfers')}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={lines.length === 0 || createMut.isPending}
          onClick={() => createMut.mutate()}
        >
          Submit request
        </Button>
      </Box>
    </FormPage>
  );
}
