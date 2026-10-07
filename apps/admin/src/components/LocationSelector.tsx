import { Alert, MenuItem, TextField } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Permission, usePermissions } from '../hooks/usePermissions';
import { panelClaims } from '../lib/authSession';
import { selectedLocationId, selectLocation } from '../lib/locationSelection';
import { getVenueLocations } from '../services/config';
export default function LocationSelector() {
  const { can } = usePermissions();
  const org = panelClaims()?.tenantId ?? '';
  const locations = useQuery({
    queryKey: ['venue-locations', org],
    queryFn: () => getVenueLocations(org),
    enabled: !!org && can(Permission.LocationsRead),
  });
  if (!can(Permission.LocationsRead)) return null;
  if (locations.error) return <Alert severity="error">Locations could not be loaded.</Alert>;
  const selected = selectedLocationId();
  return (
    <TextField
      select
      fullWidth
      size="small"
      label="Location"
      value={selected}
      onChange={(event) => selectLocation(event.target.value)}
      sx={{ my: 1 }}
      slotProps={{ select: { displayEmpty: true }, inputLabel: { shrink: true } }}
    >
      <MenuItem value="">All accessible locations</MenuItem>
      {selected && locations.data && !locations.data.some((item) => item.id === selected) && (
        <MenuItem value={selected} disabled>
          Location access changed — select another
        </MenuItem>
      )}
      {(locations.data ?? []).map((item) => (
        <MenuItem key={item.id} value={item.id}>
          {item.name}
        </MenuItem>
      ))}
    </TextField>
  );
}
