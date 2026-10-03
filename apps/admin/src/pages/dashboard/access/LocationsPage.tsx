import { PageHeader, PageShell } from '@gaming-cafe/ui';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { createMember, getAccess, saveRole } from '../../../services/access';
import type { VenueLocation } from '../../../services/config';
import LocationsPanel from './LocationsPanel';

const LOCATION_ADMIN_ROLE = 'Location administrator';
const LOCATION_ADMIN_PERMISSIONS = [
  'locations:read',
  'devices:read',
  'devices:write',
  'settings:read',
  'rules:read',
];

export default function LocationsPage() {
  const { can } = usePermissions();
  const client = useQueryClient();
  const canManageAdmins = can(Permission.AccessManage);
  const [location, setLocation] = useState<VenueLocation | null>(null);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const access = useQuery({
    queryKey: ['accessManagement'],
    queryFn: getAccess,
    enabled: canManageAdmins,
  });
  const adminRoleIds = new Set(
    access.data?.roles
      .filter((role) => !role.isTemplate && role.name === LOCATION_ADMIN_ROLE)
      .map((role) => role.id) ?? [],
  );
  const adminsByLocation: Record<string, string[]> = {};
  for (const member of access.data?.members ?? []) {
    if (!member.active) continue;
    for (const scope of member.locationRoles ?? []) {
      if (!scope.roleIds.some((roleId) => adminRoleIds.has(roleId))) continue;
      const locationAdmins = adminsByLocation[scope.locationId] ?? [];
      locationAdmins.push(member.name || member.username);
      adminsByLocation[scope.locationId] = locationAdmins;
    }
  }
  const createAdmin = useMutation({
    mutationFn: async () => {
      if (!location) throw new Error('Choose a location before adding an admin.');
      const snapshot = await getAccess();
      let roleId = snapshot.roles.find(
        (role) => !role.isTemplate && role.name === LOCATION_ADMIN_ROLE,
      )?.id;
      if (!roleId) {
        const role = await saveRole({
          name: LOCATION_ADMIN_ROLE,
          description:
            'Starter permissions for assigned venue locations. Edit this role in Access management.',
          permissions: LOCATION_ADMIN_PERMISSIONS,
          isTemplate: false,
        });
        roleId = role.id;
      }
      return createMember({
        username: username.trim(),
        password,
        roleIds: [roleId],
        locationIds: [location.id],
        locationRoles: [{ locationId: location.id, roleIds: [roleId] }],
      });
    },
    onSuccess: () => {
      setLocation(null);
      setUsername('');
      setPassword('');
      void client.invalidateQueries({ queryKey: ['accessManagement'] });
    },
  });
  const validUsername = /^[a-zA-Z0-9_.-]{3,80}$/.test(username.trim());
  const passwordBytes = new TextEncoder().encode(password).length;
  return (
    <PageShell>
      <PageHeader
        title="Locations"
        description="Create venues and assign administrators to each location."
      />
      {canManageAdmins && (
        <Button component={Link} to="/access?tab=team" sx={{ mb: 1 }}>
          Manage existing team access
        </Button>
      )}
      <LocationsPanel
        canWrite={can(Permission.LocationsManage)}
        adminsByLocation={adminsByLocation}
        onAddAdmin={
          canManageAdmins && access.data
            ? (selected) => {
                createAdmin.reset();
                setLocation(selected);
              }
            : undefined
        }
      />
      {access.error && (
        <Alert severity="error" sx={{ mt: 2 }}>
          Could not load team access: {access.error.message}
        </Alert>
      )}
      <Dialog
        open={!!location}
        onClose={() => !createAdmin.isPending && setLocation(null)}
        fullWidth
        maxWidth="sm"
      >
        <DialogTitle>Add location admin</DialogTitle>
        <DialogContent>
          <Stack gap={2} sx={{ pt: 1 }}>
            <Typography>
              Assign a new administrator to {location?.name}. Their starter role can be changed in
              Access management.
            </Typography>
            <TextField
              label="Username"
              required
              autoComplete="off"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              helperText="3–80 letters, numbers, dots, underscores, or hyphens"
            />
            <TextField
              label="Initial password"
              required
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              helperText="12–72 bytes. Share securely with this team member."
            />
            {createAdmin.error && <Alert severity="error">{createAdmin.error.message}</Alert>}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button disabled={createAdmin.isPending} onClick={() => setLocation(null)}>
            Cancel
          </Button>
          <Button
            variant="contained"
            disabled={
              createAdmin.isPending || !validUsername || passwordBytes < 12 || passwordBytes > 72
            }
            onClick={() => createAdmin.mutate()}
          >
            Create admin
          </Button>
        </DialogActions>
      </Dialog>
    </PageShell>
  );
}
