import {
  Alert,
  Autocomplete,
  Box,
  Button,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'react-toastify';
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
      toast.success('Waste recorded — pending admin approval');
      navigate('/inventory/waste');
    },
    onError: () => toast.error('Failed to record waste'),
  });

  const needsNote = reasonCode === 'other';
  const canSubmit =
    locationId &&
    productId &&
    Number(quantityPieces) > 0 &&
    (!needsNote || lineNote.trim().length >= 10);

  return (
    <Paper sx={{ p: 4 }}>
      <Typography variant="h5" fontWeight={600} gutterBottom>
        Record stock waste
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        Quantities in pieces. Admin must approve before stock is deducted.
      </Typography>

      {activeLocations.length === 0 && (
        <Alert severity="warning" sx={{ mb: 2 }}>
          No inventory locations configured.
        </Alert>
      )}

      <Box sx={{ display: 'grid', gap: 2, maxWidth: 560 }}>
        <TextField
          select
          label="Location"
          value={locationId}
          onChange={(e) => setLocationId(e.target.value)}
          required
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
          renderInput={(params) => <TextField {...params} label="Product" required />}
        />

        <TextField
          type="number"
          label="Quantity (pieces)"
          value={quantityPieces}
          onChange={(e) => setQuantityPieces(e.target.value)}
          inputProps={{ min: 1 }}
          required
        />

        <TextField
          select
          label="Reason"
          value={reasonCode}
          onChange={(e) => setReasonCode(e.target.value)}
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
        />

        <TextField
          label="Event notes (optional)"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          multiline
          rows={2}
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
    </Paper>
  );
}
