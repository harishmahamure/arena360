import { LineItemEditor, WizardPage } from '@gaming-cafe/ui';
import { Delete } from '@mui/icons-material';
import {
  Autocomplete,
  Box,
  IconButton,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  createPurchaseOrder,
  getInventoryLocations,
  type PurchaseOrderLineInput,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { getVendors } from '../../../services/vendors';

interface DraftLine extends PurchaseOrderLineInput {
  key: string;
  product?: ProductResponse | null;
}
const newLine = (): DraftLine => ({
  key: crypto.randomUUID(),
  productId: '',
  orderedBoxes: 1,
  boxCost: 0,
  taxRate: 0,
  product: null,
});
const currency = new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' });

export default function PurchaseOrderNewPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [step, setStep] = useState(0);
  const [vendorId, setVendorId] = useState('');
  const [locationId, setLocationId] = useState('');
  const [expectedDate, setExpectedDate] = useState('');
  const [notes, setNotes] = useState('');
  const [discount, setDiscount] = useState(0);
  const [freight, setFreight] = useState(0);
  const [lines, setLines] = useState<DraftLine[]>([newLine()]);
  const [suggestionSeeded, setSuggestionSeeded] = useState(false);
  const vendors = useQuery({
    queryKey: ['vendors-active'],
    queryFn: () => getVendors({ limit: 100, isActive: true }),
  });
  const locations = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 100 }),
  });
  const products = useQuery({
    queryKey: ['products-po'],
    queryFn: () => getProducts({ limit: 500, sortBy: 'name', sortOrder: 'ASC' }),
  });
  useEffect(() => {
    if (suggestionSeeded || !products.data) return;
    const productId = searchParams.get('productId');
    const suggestedPieces = Number(searchParams.get('pieces') ?? 0);
    const suggestedProduct = products.data.data.find((item) => item.id === productId);
    if (suggestedProduct && suggestedPieces > 0) {
      const unitsPerBox = Math.max(1, suggestedProduct.unitsPerPurchaseUnit ?? 1);
      setLines([
        {
          key: crypto.randomUUID(),
          productId: suggestedProduct.id,
          product: suggestedProduct,
          orderedBoxes: Math.ceil(suggestedPieces / unitsPerBox),
          boxCost: Number(suggestedProduct.purchasePricePerBox ?? 0),
          taxRate: 0,
        },
      ]);
      setLocationId(searchParams.get('locationId') ?? '');
      setVendorId(searchParams.get('vendorId') ?? '');
    }
    setSuggestionSeeded(true);
  }, [products.data, searchParams, suggestionSeeded]);
  const totals = useMemo(() => {
    const subtotal = lines.reduce((sum, line) => sum + line.orderedBoxes * line.boxCost, 0);
    const tax = lines.reduce(
      (sum, line) => sum + (line.orderedBoxes * line.boxCost * (line.taxRate ?? 0)) / 100,
      0,
    );
    return { subtotal, tax, total: Math.max(0, subtotal - discount + tax + freight) };
  }, [lines, discount, freight]);
  const mutation = useMutation({
    mutationFn: () =>
      createPurchaseOrder({
        vendorId,
        destinationLocationId: locationId,
        expectedDeliveryDate: expectedDate || undefined,
        discount,
        freight,
        notes: notes || undefined,
        lines: lines.map(({ key: _key, product: _product, ...line }) => line),
      }),
    onSuccess: (order) => navigate(`/inventory/purchase-orders/${order.id}`),
  });
  const updateLine = (key: string, patch: Partial<DraftLine>) =>
    setLines((current) => current.map((line) => (line.key === key ? { ...line, ...patch } : line)));
  const validReferences = Boolean(vendorId && locationId);
  const validLines =
    lines.length > 0 &&
    lines.every((line) => line.productId && line.orderedBoxes > 0 && line.boxCost >= 0);
  const uniqueProducts =
    new Set(lines.map((line) => line.productId).filter(Boolean)).size === lines.length;
  const next = () => {
    if (step < 2) setStep((value) => value + 1);
    else mutation.mutate();
  };

  return (
    <WizardPage
      title="Create purchase order"
      steps={['Supplier & destination', 'Line items', 'Review']}
      activeStep={step}
      onBack={
        step ? () => setStep((value) => value - 1) : () => navigate('/inventory/purchase-orders')
      }
      onNext={next}
      nextLabel={step === 2 ? 'Create draft' : 'Continue'}
      nextDisabled={step === 0 ? !validReferences : !validLines || !uniqueProducts}
      busy={mutation.isPending}
      error={mutation.error instanceof Error ? mutation.error.message : undefined}
    >
      {step === 0 ? (
        <Stack spacing={2} maxWidth={680}>
          <TextField
            select
            label="Vendor"
            value={vendorId}
            onChange={(event) => setVendorId(event.target.value)}
            required
          >
            {vendors.data?.data
              .filter((vendor) => vendor.isActive)
              .map((vendor) => (
                <MenuItem key={vendor.id} value={vendor.id}>
                  {vendor.name}
                </MenuItem>
              ))}
          </TextField>
          <TextField
            select
            label="Destination location"
            value={locationId}
            onChange={(event) => setLocationId(event.target.value)}
            required
          >
            {locations.data?.data
              .filter((location) => location.isActive)
              .map((location) => (
                <MenuItem key={location.id} value={location.id}>
                  {location.name} ({location.kind})
                </MenuItem>
              ))}
          </TextField>
          <TextField
            label="Expected delivery"
            type="date"
            value={expectedDate}
            onChange={(event) => setExpectedDate(event.target.value)}
            slotProps={{ inputLabel: { shrink: true } }}
          />
          <TextField
            label="Notes"
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            multiline
            rows={3}
          />
        </Stack>
      ) : null}
      {step === 1 ? (
        <LineItemEditor onAdd={() => setLines((current) => [...current, newLine()])}>
          {lines.map((line, index) => (
            <Paper key={line.key} variant="outlined" sx={{ p: 2 }}>
              <Stack
                direction={{ xs: 'column', md: 'row' }}
                spacing={2}
                alignItems={{ md: 'center' }}
              >
                <Autocomplete
                  sx={{ flex: 1, minWidth: 240 }}
                  options={products.data?.data ?? []}
                  value={line.product ?? null}
                  getOptionLabel={(product) => product.name}
                  onChange={(_, product) =>
                    updateLine(line.key, {
                      product,
                      productId: product?.id ?? '',
                      boxCost: Number(product?.purchasePricePerBox ?? 0),
                    })
                  }
                  renderInput={(params) => (
                    <TextField {...params} label={`Product ${index + 1}`} required />
                  )}
                />
                <TextField
                  label="Boxes"
                  type="number"
                  value={line.orderedBoxes}
                  onChange={(event) =>
                    updateLine(line.key, { orderedBoxes: Number(event.target.value) })
                  }
                  slotProps={{ htmlInput: { min: 1 } }}
                  sx={{ width: 110 }}
                />
                <TextField
                  label="Cost / box"
                  type="number"
                  value={line.boxCost}
                  onChange={(event) =>
                    updateLine(line.key, { boxCost: Number(event.target.value) })
                  }
                  slotProps={{ htmlInput: { min: 0, step: '0.01' } }}
                  sx={{ width: 150 }}
                />
                <TextField
                  label="Tax %"
                  type="number"
                  value={line.taxRate ?? 0}
                  onChange={(event) =>
                    updateLine(line.key, { taxRate: Number(event.target.value) })
                  }
                  slotProps={{ htmlInput: { min: 0, step: '0.01' } }}
                  sx={{ width: 110 }}
                />
                <IconButton
                  aria-label="Remove line"
                  disabled={lines.length === 1}
                  onClick={() =>
                    setLines((current) => current.filter((item) => item.key !== line.key))
                  }
                >
                  <Delete />
                </IconButton>
              </Stack>
            </Paper>
          ))}
        </LineItemEditor>
      ) : null}
      {step === 2 ? (
        <Stack spacing={2}>
          <Typography variant="h6">Review draft</Typography>
          {lines.map((line) => (
            <Stack key={line.key} direction="row" justifyContent="space-between">
              <Typography>
                {line.product?.name} · {line.orderedBoxes} boxes
              </Typography>
              <Typography>{currency.format(line.orderedBoxes * line.boxCost)}</Typography>
            </Stack>
          ))}
          <Box
            sx={{
              borderTop: 1,
              borderColor: 'divider',
              pt: 2,
              display: 'grid',
              gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, 180px)' },
              justifyContent: 'end',
              gap: 2,
            }}
          >
            <TextField
              label="Discount"
              type="number"
              value={discount}
              onChange={(event) => setDiscount(Number(event.target.value))}
            />
            <TextField
              label="Freight"
              type="number"
              value={freight}
              onChange={(event) => setFreight(Number(event.target.value))}
            />
          </Box>
          <Stack alignItems="flex-end">
            <Typography>Subtotal: {currency.format(totals.subtotal)}</Typography>
            <Typography>Tax: {currency.format(totals.tax)}</Typography>
            <Typography variant="h6">Total: {currency.format(totals.total)}</Typography>
          </Stack>
        </Stack>
      ) : null}
    </WizardPage>
  );
}
