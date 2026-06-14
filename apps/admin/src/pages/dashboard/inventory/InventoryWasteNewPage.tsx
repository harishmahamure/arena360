import { FormPage, IntegerField } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Alert, Autocomplete, Box, Button, MenuItem, TextField } from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { createWasteEvent, getInventoryLocations } from '../../../services/inventory';
import { getProducts } from '../../../services/product/list';

const REASON_OPTIONS = [
  { value: 'expired', label: 'Expired' },
  { value: 'damaged', label: 'Damaged' },
  { value: 'spoilage', label: 'Spoilage' },
  { value: 'sample', label: 'Sample / promo' },
  { value: 'other', label: 'Other (note required)' },
];

export default function InventoryWasteNewPage() {
  const navigate = useNavigate();
  const [locationId, setLocationId] = useState('');
  const [productId, setProductId] = useState('');
  const [quantityPieces, setQuantityPieces] = useState('1');
  const [reasonCode, setReasonCode] = useState('expired');
  const [lineNote, setLineNote] = useState('');
  const [notes, setNotes] = useState('');

  const { data: locations } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50, isActive: true }),
  });

  const { data: productsData } = useQuery({
    queryKey: ['products-waste'],
    queryFn: () => getProducts({ limit: 200 }),
  });

  const activeLocations = useMemo(
    () => (locations?.data ?? []).filter((l) => l.isActive),
    [locations],
  );

  const createMut = useMutation({
    mutationFn: () =>
      createWasteEvent({
        locationId,
        notes: notes || undefined,
        lines: [
          {
            productId,
            quantityPieces: Number(quantityPieces),
            reasonCode,
            note: lineNote || undefined,
          },
        ],
      }),
    onSuccess: () => {
      toastUtils.success('Waste recorded — pending admin approval');
      navigate('/inventory/waste');
    },
    onError: () => toastUtils.error('Failed to record waste'),
  });

  const needsNote = reasonCode === 'other';
  const canSubmit =
    locationId &&
    productId &&
    Number(quantityPieces) > 0 &&
    (!needsNote || lineNote.trim().length >= 10);

  return (
    <FormPage
      title="Record stock waste"
      description="Quantities in pieces. Admin must approve before stock is deducted."
      backTo="/inventory/waste"
      backLabel="Back to waste log"
      breadcrumbs={[{ label: 'Inventory', to: '/inventory/waste' }, { label: 'Record waste' }]}
      maxWidth={600}
    >
      {activeLocations.length === 0 && (
        <Alert severity="warning" sx={{ mb: 2 }}>
          No inventory locations configured.
        </Alert>
      )}

      <Box sx={{ display: 'grid', gap: 2 }}>
        <TextField
          select
          label="Location"
          value={locationId}
          onChange={(e) => setLocationId(e.target.value)}
          required
          helperText="Store or warehouse where the waste occurred"
        >
          {activeLocations.map((l) => (
            <MenuItem key={l.id} value={l.id}>
              {l.name} ({l.kind})
            </MenuItem>
          ))}
        </TextField>

        <Autocomplete
          options={productsData?.data ?? []}
          getOptionLabel={(p) => p.name}
          onChange={(_, p) => setProductId(p?.id ?? '')}
          renderInput={(params) => (
            <TextField
              {...params}
              label="Product"
              required
              helperText="Product being written off from inventory"
            />
          )}
        />

        <IntegerField
          label="Quantity (pieces)"
          value={quantityPieces}
          onChange={(e) => setQuantityPieces(e.target.value)}
          inputProps={{ min: 1 }}
          required
          helperText="Number of individual pieces wasted"
        />

        <TextField
          select
          label="Reason"
          value={reasonCode}
          onChange={(e) => setReasonCode(e.target.value)}
          helperText="Why the stock is being removed"
        >
          {REASON_OPTIONS.map((r) => (
            <MenuItem key={r.value} value={r.value}>
              {r.label}
            </MenuItem>
          ))}
        </TextField>

        <TextField
          label={needsNote ? 'Detail (min 10 chars)' : 'Line note (optional)'}
          value={lineNote}
          onChange={(e) => setLineNote(e.target.value)}
          multiline
          rows={2}
          required={needsNote}
          helperText={
            needsNote
              ? 'Required when reason is Other — describe what happened (min 10 chars)'
              : 'Optional detail for this line item'
          }
        />

        <TextField
          label="Event notes (optional)"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          multiline
          rows={2}
          helperText="Optional notes for the whole waste event"
        />
      </Box>

      <Box sx={{ mt: 3, display: 'flex', gap: 2 }}>
        <Button variant="outlined" onClick={() => navigate('/inventory/waste')}>
          Cancel
        </Button>
        <Button
          variant="contained"
          disabled={!canSubmit || createMut.isPending}
          onClick={() => createMut.mutate()}
        >
          Submit for approval
        </Button>
      </Box>
    </FormPage>
  );
}
