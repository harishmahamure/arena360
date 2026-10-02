import { Nightlight, Store as StoreIcon } from '@mui/icons-material';
import { Chip, InputAdornment, MenuItem, Stack, TextField, Tooltip } from '@mui/material';

export interface PosStoreToolbarProps {
  saleLocationId: string;
  storeLocations: { id: string; name: string }[];
  nightActive?: boolean;
  onLocationChange: (id: string) => void;
}

/** Compact store picker that sits beside the product search; changing store clears the cart. */
export function PosStoreToolbar({
  saleLocationId,
  storeLocations,
  nightActive = false,
  onLocationChange,
}: PosStoreToolbarProps) {
  return (
    <Stack direction="row" spacing={1} alignItems="center" sx={{ flexShrink: 0 }}>
      <Tooltip title="Stock is sold from this store. Changing store clears the cart.">
        <TextField
          select
          aria-label="Store"
          value={saleLocationId}
          onChange={(e) => onLocationChange(e.target.value)}
          sx={{ minWidth: 180 }}
          slotProps={{
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <StoreIcon fontSize="small" />
                </InputAdornment>
              ),
            },
          }}
        >
          {storeLocations.map((loc) => (
            <MenuItem key={loc.id} value={loc.id}>
              {loc.name}
            </MenuItem>
          ))}
        </TextField>
      </Tooltip>
      {nightActive && (
        <Tooltip title="Night price active (11 PM – 8 AM)">
          <Chip icon={<Nightlight />} label="Night" color="secondary" />
        </Tooltip>
      )}
    </Stack>
  );
}
