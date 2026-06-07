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
import { useState } from 'react';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  createInventoryLocation,
  getInventoryLocations,
  type InventoryLocation,
  updateInventoryLocation,
} from '../../../services/inventory';

export default function InventoryLocationsPage() {
  const { can } = usePermissions();
  const canManage = can(Permission.InventoryManage);
  const queryClient = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<InventoryLocation | null>(null);
  const [name, setName] = useState('');
  const [kind, setKind] = useState<'warehouse' | 'store'>('store');
  const [isActive, setIsActive] = useState(true);
  const [search, setSearch] = useState('');

  const { data, isLoading, error } = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 50 }),
  });

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (editing) {
        return updateInventoryLocation(editing.id, { name, kind, isActive });
      }
      return createInventoryLocation({ name, kind, isActive });
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
    setDialogOpen(true);
  };

  const openEdit = (loc: InventoryLocation) => {
    setEditing(loc);
    setName(loc.name);
    setKind(loc.kind);
    setIsActive(loc.isActive);
    setDialogOpen(true);
  };

  const columns: Column<InventoryLocation>[] = [
    { id: 'name', label: 'Name', minWidth: 180 },
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

      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>{editing ? 'Edit Location' : 'Add Location'}</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
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
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDialogOpen(false)}>Cancel</Button>
          <Button
            variant="contained"
            disabled={!name.trim() || saveMutation.isPending}
            onClick={() => saveMutation.mutate()}
          >
            Save
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
