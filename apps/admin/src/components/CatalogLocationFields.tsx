import {
  Alert,
  Autocomplete,
  Box,
  Checkbox,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { panelClaims } from '../lib/authSession';
import { selectedLocationId } from '../lib/locationSelection';
import { getMyAccess } from '../services/access';
import { getVenueLocations } from '../services/config';
export function useCatalogLocations() {
  const org = panelClaims()?.tenantId ?? '';
  const access = useQuery({ queryKey: ['myAccess'], queryFn: getMyAccess });
  const locations = useQuery({
    queryKey: ['venue-locations', org],
    queryFn: () => getVenueLocations(org),
    enabled: !!org,
  });
  return {
    locations: locations.data ?? [],
    organizationAdmin: access.data?.organizationAdmin ?? false,
    loading: access.isLoading || locations.isLoading,
    error: access.error ?? locations.error,
  };
}
export default function CatalogLocationFields({
  value,
  onChange,
  disabled = false,
}: {
  value: string[] | undefined;
  onChange: (ids: string[]) => void;
  disabled?: boolean;
}) {
  const { locations, organizationAdmin, loading, error } = useCatalogLocations();
  const shared = value?.length === 0;
  return (
    <Box sx={{ my: 3 }}>
      <Typography variant="h6">Location availability</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
        Use one catalog entry across locations. Configure different prices on its detail page.
      </Typography>
      {error && <Alert severity="error">{error.message}</Alert>}
      <Stack gap={1}>
        {organizationAdmin && (
          <FormControlLabel
            label="Shared across all locations"
            control={
              <Checkbox
                checked={shared || value === undefined}
                disabled={disabled || loading}
                onChange={(e) =>
                  onChange(e.target.checked ? [] : locations.slice(0, 1).map((l) => l.id))
                }
              />
            }
          />
        )}
        {(!organizationAdmin || (!shared && value !== undefined)) && (
          <Autocomplete
            multiple
            options={locations}
            getOptionLabel={(l) => l.name}
            value={locations.filter((l) =>
              (
                value ??
                (selectedLocationId() ? [selectedLocationId()] : locations.map((item) => item.id))
              ).includes(l.id),
            )}
            disabled={disabled || loading}
            onChange={(_, items) => onChange(items.map((l) => l.id))}
            isOptionEqualToValue={(a, b) => a.id === b.id}
            renderInput={(params) => (
              <TextField
                {...params}
                label="Available at locations"
                helperText="Choose one or more locations"
              />
            )}
          />
        )}
      </Stack>
    </Box>
  );
}
