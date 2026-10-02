import { Nightlight, Store as StoreIcon } from '@mui/icons-material';
import { Box, Chip, FormHelperText, MenuItem, TextField } from '@mui/material';

export interface PosStoreToolbarProps {
  saleLocationId: string;
  storeLocations: { id: string; name: string }[];
  nightActive?: boolean;
  onLocationChange: (id: string) => void;
  helperText?: string;
}

export function PosStoreToolbar({
  saleLocationId,
  storeLocations,
  nightActive = false,
  onLocationChange,
  helperText = 'Which store location stock is sold from; changing store clears the cart',
}: PosStoreToolbarProps) {
  return (
    <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'flex-start' }}>
      <StoreIcon color="action" sx={{ mt: 1 }} />
      <Box>
        <TextField
          select
          label="Store"
          size="small"
          value={saleLocationId}
          onChange={(e) => onLocationChange(e.target.value)}
          sx={{ minWidth: 220 }}
        >
          {storeLocations.map((loc) => (
            <MenuItem key={loc.id} value={loc.id}>
              {loc.name}
            </MenuItem>
          ))}
        </TextField>
        <FormHelperText sx={{ mx: 0 }}>{helperText}</FormHelperText>
      </Box>
      {nightActive && (
        <Chip
          icon={<Nightlight />}
          label="Night price active (11 PM – 8 AM)"
          color="secondary"
          size="small"
          sx={{ mt: 0.5 }}
        />
      )}
    </Box>
  );
}
