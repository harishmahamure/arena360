import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Edit } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { GuidedForm, GuidedStep } from '../../../components/GuidedForm';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getVenueLocations } from '../../../services/config';
import {
  createInventoryLocation,
  getInventoryLocations,
  type InventoryLocation,
  updateInventoryLocation,
} from '../../../services/inventory';
import { currentOrganizationId } from '../access/LocationsPanel';

export default function InventoryLocationsPage() {
  const { can } = usePermissions();
  const canManage = can(Permission.InventoryManage);
  const queryClient = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<InventoryLocation | null>(null);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<'warehouse' | 'store'>('store');
  const [isActive, setIsActive] = useState(true);
  const [venueLocationId, setVenueLocationId] = useState('');
  const [filterVenueId, setFilterVenueId] = useState('');
  const [search, setSearch] = useState('');

  const organizationId = currentOrganizationId();
  const venues = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
  });
  useEffect(() => {
    if (!filterVenueId && venues.data?.length) setFilterVenueId(venues.data[0]?.id ?? '');
  }, [filterVenueId, venues.data]);

  const { data, isLoading, error } = useQuery({
    queryKey: ['inventory-locations', filterVenueId],
    queryFn: () => getInventoryLocations({ limit: 50, venueLocationId: filterVenueId }),
    enabled: !!filterVenueId,
  });

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (editing) {
        return updateInventoryLocation(editing.id, { name, kind, isActive, venueLocationId });
      }
      return createInventoryLocation({ name, kind, isActive, venueLocationId });
    },
    onSuccess: () => {
      toastUtils.success(editing ? 'Location updated' : 'Location created');
      queryClient.invalidateQueries({ queryKey: ['inventory-locations'] });
      setDialogOpen(false);
    },
    onError: () => toastUtils.error('Failed to save location'),
  });

  const openCreate = () => {
    setEditing(null);
    setName('');
    setKind('store');
    setIsActive(true);
    setVenueLocationId(filterVenueId);
    setDialogOpen(true);
  };

  const openEdit = (loc: InventoryLocation) => {
    setEditing(loc);
    setName(loc.name);
    setKind(loc.kind);
    setIsActive(loc.isActive);
    setVenueLocationId(loc.venueLocationId ?? filterVenueId);
    setDialogOpen(true);
  };

  const columns: Column<InventoryLocation>[] = [
    { id: 'name', label: 'Name', minWidth: 180 },
    {
      id: 'venueLocationId',
      label: 'Venue',
      minWidth: 140,
      format: (value) => venues.data?.find((venue) => venue.id === value)?.name ?? '—',
    },
    {
      id: 'kind',
      label: 'Type',
      minWidth: 120,
      format: (v) => (
        <Chip
          label={(v as string) === 'warehouse' ? 'Warehouse' : 'Store'}
          size="small"
          color={(v as string) === 'warehouse' ? 'primary' : 'secondary'}
          variant="outlined"
        />
      ),
    },
    {
      id: 'isActive',
      label: 'Active',
      minWidth: 80,
      format: (v) => (
        <Chip label={v ? 'Active' : 'Inactive'} size="small" color={v ? 'success' : 'default'} />
      ),
    },
  ];

  const actions: Action<InventoryLocation>[] = canManage
    ? [{ label: 'Edit', icon: <Edit fontSize="small" />, onClick: (row) => openEdit(row) }]
    : [];

  const filtered =
    data?.data.filter((l) => l.name.toLowerCase().includes(search.toLowerCase())) ?? [];

  return (
    <>
      <TextField
        select
        size="small"
        label="Venue"
        value={filterVenueId}
        onChange={(event) => setFilterVenueId(event.target.value)}
        sx={{ mb: 2, mx: { xs: 2, md: 4 }, minWidth: 220 }}
      >
        {venues.data?.map((venue) => (
          <MenuItem key={venue.id} value={venue.id}>
            {venue.name}
          </MenuItem>
        ))}
      </TextField>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load locations
        </Alert>
      )}
      <ListPage
        title="Inventory Locations"
        description="Configure warehouse and store locations for stock tracking"
        columns={columns}
        data={filtered}
        actions={actions}
        isLoading={isLoading}
        showSearch
        searchValue={search}
        onSearchChange={(e) => setSearch(e.target.value)}
        onSearchClear={() => setSearch('')}
        onAddClick={canManage ? openCreate : undefined}
        addButtonLabel="Add Location"
      />

      <Dialog
        open={dialogOpen}
        onClose={saveMutation.isPending ? undefined : () => setDialogOpen(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>{editing ? 'Edit Location' : 'Add Location'}</DialogTitle>
        <DialogContent>
          <GuidedForm
            busy={saveMutation.isPending}
            onCancel={() => setDialogOpen(false)}
            actions={
              <DialogActions>
                <Button data-wizard-cancel onClick={() => setDialogOpen(false)}>
                  Cancel
                </Button>
                <Button
                  variant="contained"
                  disabled={!name.trim() || !venueLocationId || saveMutation.isPending}
                  onClick={() => saveMutation.mutate()}
                >
                  Save
                </Button>
              </DialogActions>
            }
          >
            <GuidedStep
              title="Location details"
              validate={() =>
                !name.trim()
                  ? 'Enter a location name.'
                  : !venueLocationId
                    ? 'Select a venue.'
                    : undefined
              }
            >
              <TextField
                select
                label="Venue"
                value={venueLocationId}
                onChange={(e) => setVenueLocationId(e.target.value)}
                fullWidth
                required
              >
                {venues.data?.map((venue) => (
                  <MenuItem key={venue.id} value={venue.id}>
                    {venue.name}
                  </MenuItem>
                ))}
              </TextField>
              <TextField
                label="Name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                fullWidth
                required
              />
              <TextField
                select
                label="Type"
                value={kind}
                onChange={(e) => setKind(e.target.value as 'warehouse' | 'store')}
                fullWidth
              >
                <MenuItem value="warehouse">Warehouse</MenuItem>
                <MenuItem value="store">Store</MenuItem>
              </TextField>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                <Switch checked={isActive} onChange={(e) => setIsActive(e.target.checked)} />
                <Typography variant="body2">Active</Typography>
              </Box>
            </GuidedStep>
          </GuidedForm>
        </DialogContent>
      </Dialog>
    </>
  );
}
