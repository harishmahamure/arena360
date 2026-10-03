import { local } from '@gaming-cafe/utils';
import { Add } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { decodeJwtPayload } from '../../../lib/authSession';
import {
  createVenueLocation,
  getVenueLocations,
  updateVenueLocation,
  type VenueLocation,
  type VenueLocationDraft,
} from '../../../services/config';

export function currentOrganizationId() {
  const token = local.get<string>('accessToken');
  return (
    (token ? decodeJwtPayload(token)?.tenantId : undefined) ??
    '00000000-0000-4000-8000-000000000001'
  );
}

const empty: VenueLocationDraft = { slug: '', name: '', timezone: 'Asia/Kolkata', currency: 'INR' };

export default function LocationsPanel({
  canWrite,
  onAddAdmin,
  adminsByLocation = {},
}: {
  canWrite: boolean;
  onAddAdmin?: (location: VenueLocation) => void;
  adminsByLocation?: Record<string, string[]>;
}) {
  const organizationId = currentOrganizationId();
  const client = useQueryClient();
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId, canWrite ? 'management' : 'active'],
    queryFn: () => getVenueLocations(organizationId, canWrite),
  });
  const [editing, setEditing] = useState<VenueLocation | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<VenueLocationDraft>(empty);
  const save = useMutation({
    mutationFn: () =>
      editing
        ? updateVenueLocation(organizationId, editing.id, draft)
        : createVenueLocation(organizationId, draft),
    onSuccess: () => {
      setEditing(null);
      setCreating(false);
      setDraft(empty);
      void client.invalidateQueries({ queryKey: ['venue-locations', organizationId] });
      void client.invalidateQueries({ queryKey: ['accessManagement'] });
    },
  });
  const beginEdit = (location: VenueLocation) => {
    setEditing(location);
    setCreating(false);
    setDraft({
      slug: location.slug,
      name: location.name,
      timezone: location.timezone,
      currency: location.currency,
      isActive: location.isActive,
    });
    save.reset();
  };
  return (
    <Stack gap={2} sx={{ mt: 2 }}>
      <Stack direction={{ xs: 'column', sm: 'row' }} gap={2} alignItems={{ sm: 'center' }}>
        <Typography color="text.secondary" sx={{ flex: 1 }}>
          Organization pricing applies to all venues until a location-specific rule is published.
        </Typography>
        {canWrite && (
          <Button
            variant="contained"
            startIcon={<Add />}
            onClick={() => {
              setEditing(null);
              setDraft(empty);
              setCreating(true);
              save.reset();
            }}
          >
            Create location
          </Button>
        )}
      </Stack>
      {locations.error && <Alert severity="error">{locations.error.message}</Alert>}
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: { xs: '1fr', md: 'repeat(2,minmax(0,1fr))' },
          gap: 2,
        }}
      >
        {locations.data?.map((location) => (
          <Card key={location.id} variant="outlined">
            <CardContent>
              <Typography variant="h6">{location.name}</Typography>
              <Typography color="text.secondary">
                {location.slug} · {location.timezone} · {location.currency} ·{' '}
                {location.isActive ? 'Active' : 'Inactive'}
              </Typography>
              {adminsByLocation[location.id]?.length ? (
                <Typography variant="body2" sx={{ mt: 1 }}>
                  Location admins: {adminsByLocation[location.id]?.join(', ')}
                </Typography>
              ) : null}
              <Stack direction="row" gap={1} sx={{ mt: 1 }}>
                {canWrite && <Button onClick={() => beginEdit(location)}>Edit location</Button>}
                {location.isActive && onAddAdmin && (
                  <Button onClick={() => onAddAdmin(location)}>Add location admin</Button>
                )}
              </Stack>
            </CardContent>
          </Card>
        ))}
      </Box>
      {canWrite && (creating || editing) && (
        <Card variant="outlined">
          <CardContent>
            <Stack gap={2}>
              <Typography variant="h6">
                {editing ? `Edit ${editing.name}` : 'Create location'}
              </Typography>
              <TextField
                label="Location name"
                required
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
              <TextField
                label="Slug"
                required
                helperText="Lowercase letters, numbers, and hyphens"
                value={draft.slug}
                onChange={(e) => setDraft({ ...draft, slug: e.target.value })}
              />
              <TextField
                label="IANA timezone"
                required
                value={draft.timezone}
                onChange={(e) => setDraft({ ...draft, timezone: e.target.value })}
              />
              <TextField
                label="Currency code"
                required
                value={draft.currency}
                onChange={(e) => setDraft({ ...draft, currency: e.target.value.toUpperCase() })}
              />
              {editing && (
                <FormControlLabel
                  label="Active"
                  control={
                    <Switch
                      checked={draft.isActive ?? true}
                      onChange={(_, isActive) => setDraft({ ...draft, isActive })}
                    />
                  }
                />
              )}
              {save.error && <Alert severity="error">{save.error.message}</Alert>}
              <Stack direction="row" gap={1}>
                <Button
                  variant="contained"
                  disabled={save.isPending || !draft.name.trim() || !draft.slug.trim()}
                  onClick={() => save.mutate()}
                >
                  {editing ? 'Save location' : 'Add location'}
                </Button>
                <Button
                  onClick={() => {
                    setEditing(null);
                    setCreating(false);
                    setDraft(empty);
                    save.reset();
                  }}
                >
                  Cancel
                </Button>
              </Stack>
            </Stack>
          </CardContent>
        </Card>
      )}
    </Stack>
  );
}
