import { PageHeader, PageShell } from '@gaming-cafe/ui';
import { Add, ContentCopy, Refresh } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  LinearProgress,
  MenuItem,
  Stack,
  Switch,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { GuidedForm, GuidedStep } from '../../../components/GuidedForm';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  type AccessMember,
  type AccessRole,
  createMember,
  deleteRole,
  effectiveGrants,
  getAccess,
  type ModuleState,
  normalizeGrants,
  type RoleDraft,
  saveMember,
  saveModule,
  saveRole,
} from '../../../services/access';
import { getVenueLocations } from '../../../services/config';
import LocationsPanel, { currentOrganizationId } from './LocationsPanel';

export default function AccessPage() {
  const { can } = usePermissions();
  const manage = can(Permission.AccessManage);
  const canManageLocations = can(Permission.LocationsManage);
  const organizationId = currentOrganizationId();
  const client = useQueryClient();
  const query = useQuery({ queryKey: ['accessManagement'], queryFn: getAccess });
  const data = query.data;
  const [searchParams] = useSearchParams();
  const [tab, setTab] = useState(() => (searchParams.get('tab') === 'team' ? 'team' : 'roles'));
  const [search, setSearch] = useState('');
  const [role, setRole] = useState<RoleDraft | null>(null);
  const [member, setMember] = useState<AccessMember | null>(null);
  const [newMember, setNewMember] = useState<{
    username: string;
    password: string;
    roleIds: string[];
    locationIds: string[];
    locationRoles: { locationId: string; roleIds: string[] }[];
  } | null>(null);
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
    enabled: !!member || !!newMember,
  });
  const [module, setModule] = useState<ModuleState | null>(null);
  const [deleting, setDeleting] = useState<AccessRole | null>(null);
  const [permissionSearch, setPermissionSearch] = useState('');
  const [templateId, setTemplateId] = useState('');
  const mutation = useMutation({
    mutationFn: async (operation: () => Promise<unknown>) => operation(),
    onSuccess: () => {
      setRole(null);
      setMember(null);
      setNewMember(null);
      setModule(null);
      setDeleting(null);
      void client.invalidateQueries();
    },
    onError: () => {
      void query.refetch();
    },
  });
  function reset() {
    mutation.reset();
    setPermissionSearch('');
    setTemplateId('');
  }
  function close() {
    if (!mutation.isPending) {
      setRole(null);
      setMember(null);
      setNewMember(null);
      setModule(null);
      setDeleting(null);
      mutation.reset();
    }
  }
  function edit(item: AccessRole, copy = false, template = item.isTemplate) {
    reset();
    setRole({
      id: copy ? undefined : item.id,
      name: copy ? `${item.name} copy` : item.name,
      description: item.description,
      permissions: [...item.permissions],
      isTemplate: template,
      expectedRevision: copy ? undefined : item.revision,
    });
  }
  const labels = new Map(
    data?.catalog.flatMap((m) => m.permissions.map((p) => [p.key, p.label] as const)),
  );
  const roleNames = (ids: string[]) =>
    ids.map((id) => data?.roles.find((r) => r.id === id)?.name ?? 'Deleted role').join(', ') ||
    'No roles';
  const selectedIds = member?.roleIds ?? newMember?.roleIds ?? [];
  const grants = data ? effectiveGrants(selectedIds, data.roles, data.modules) : [];
  function toggleRole(id: string) {
    const roleIds = selectedIds.includes(id)
      ? selectedIds.filter((v) => v !== id)
      : [...selectedIds, id];
    if (member)
      setMember({
        ...member,
        roleIds,
        locationRoles: member.locationIds.map((locationId) => ({ locationId, roleIds })),
      });
    if (newMember)
      setNewMember({
        ...newMember,
        roleIds,
        locationRoles: newMember.locationIds.map((locationId) => ({ locationId, roleIds })),
      });
  }
  const error = mutation.error && (
    <Alert severity="error" sx={{ my: 2 }}>
      {mutation.error.message} Close and reopen the editor to load newer changes.
    </Alert>
  );
  return (
    <PageShell>
      <PageHeader
        title="Access management"
        description="Manage locations, assign team access, and configure roles and modules."
      />
      <Stack direction="row" gap={1} flexWrap="wrap" sx={{ mb: 2 }}>
        <Chip label={`${data?.roles.filter((r) => !r.isTemplate).length ?? '…'} roles`} />
        <Chip label={`${data?.members.length ?? '…'} team members`} />
        <Box sx={{ flex: 1 }} />
        <Button
          startIcon={<Refresh />}
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          Refresh
        </Button>
        {manage && tab !== 'locations' && (
          <Button
            variant="contained"
            startIcon={<Add />}
            onClick={() => {
              reset();
              if (tab === 'team')
                setNewMember({
                  username: '',
                  password: '',
                  roleIds: [],
                  locationIds: [],
                  locationRoles: [],
                });
              else
                setRole({
                  name: '',
                  description: '',
                  permissions: [],
                  isTemplate: tab === 'templates',
                });
            }}
          >
            {tab === 'team'
              ? 'Add team member'
              : tab === 'templates'
                ? 'Create template'
                : 'Create role'}
          </Button>
        )}
      </Stack>
      <Alert severity="info" sx={{ mb: 2 }}>
        Permissions from assigned roles are combined. Disabled modules override role grants. Changes
        apply to this organization and affected sessions must sign in again.
      </Alert>
      <Tabs
        value={tab}
        variant="scrollable"
        allowScrollButtonsMobile
        onChange={(_, v) => {
          setTab(v);
          setSearch('');
        }}
      >
        <Tab label="Roles" value="roles" />
        <Tab label="Templates" value="templates" />
        <Tab label="Team" value="team" />
        <Tab label="Locations" value="locations" />
        <Tab label="Modules" value="modules" />
        <Tab label="Audit trail" value="audit" />
      </Tabs>
      {query.isFetching && <LinearProgress aria-label="Loading access management" />}
      {query.error && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {query.error.message}
        </Alert>
      )}
      <TextField
        fullWidth
        label={`Search ${tab === 'audit' ? 'audit trail' : tab}`}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        sx={{ my: 2 }}
      />
      {data && (
        <>
          {tab === 'locations' && <LocationsPanel canWrite={canManageLocations} />}
          {(tab === 'roles' || tab === 'templates') && (
            <>
              <Typography color="text.secondary" sx={{ mb: 2 }}>
                {tab === 'templates'
                  ? 'Templates are starting points. Updating a template does not change roles already created from it.'
                  : 'Assign one or more roles to each team member. Templates must be copied into a role before assignment.'}
              </Typography>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: '1fr', md: 'repeat(2,minmax(0,1fr))' },
                  gap: 2,
                }}
              >
                {data.roles
                  .filter(
                    (r) =>
                      r.isTemplate === (tab === 'templates') &&
                      `${r.name} ${r.description}`.toLowerCase().includes(search.toLowerCase()),
                  )
                  .map((r) => (
                    <Card key={r.id} variant="outlined">
                      <CardContent>
                        <Typography variant="h6" component="h2" sx={{ overflowWrap: 'anywhere' }}>
                          {r.name}
                        </Typography>
                        <Typography color="text.secondary" sx={{ my: 1 }}>
                          {r.description || 'Custom permission set'}
                        </Typography>
                        <Stack direction="row" gap={1} flexWrap="wrap">
                          <Chip size="small" label={`${r.permissions.length} permissions`} />
                          {!r.isTemplate && (
                            <Chip size="small" label={`${r.memberCount} members`} />
                          )}
                        </Stack>
                        <Stack direction="row" gap={1} flexWrap="wrap" sx={{ mt: 2 }}>
                          <Button onClick={() => edit(r)}>
                            {manage ? 'Edit' : 'View permissions'}
                          </Button>
                          {manage && (
                            <>
                              <Button
                                startIcon={<ContentCopy />}
                                onClick={() => edit(r, true, false)}
                              >
                                {r.isTemplate ? 'Create role' : 'Duplicate'}
                              </Button>
                              {!r.isTemplate && (
                                <Button onClick={() => edit(r, true, true)}>
                                  Save as template
                                </Button>
                              )}
                              <Button
                                color="error"
                                disabled={r.memberCount > 0}
                                onClick={() => {
                                  reset();
                                  setDeleting(r);
                                }}
                              >
                                Delete
                              </Button>
                            </>
                          )}
                        </Stack>
                      </CardContent>
                    </Card>
                  ))}
              </Box>
            </>
          )}
          {tab === 'team' && (
            <Stack gap={1}>
              {data.members
                .filter((m) =>
                  `${m.username} ${m.name} ${roleNames(m.roleIds)}`
                    .toLowerCase()
                    .includes(search.toLowerCase()),
                )
                .map((m) => (
                  <Card key={m.id} variant="outlined">
                    <CardContent>
                      <Stack
                        direction={{ xs: 'column', sm: 'row' }}
                        gap={2}
                        alignItems={{ sm: 'center' }}
                      >
                        <Box sx={{ flex: 1, minWidth: 0 }}>
                          <Typography fontWeight={700}>
                            {m.name || m.username}{' '}
                            <Chip
                              size="small"
                              label={m.active ? 'Active' : 'Access disabled'}
                              color={m.active ? 'success' : 'default'}
                            />
                          </Typography>
                          <Typography color="text.secondary">
                            @{m.username} · {roleNames(m.roleIds)}
                          </Typography>
                          <Typography variant="caption">
                            {effectiveGrants(m.roleIds, data.roles, data.modules).length} effective
                            permissions · {m.locationIds?.length ?? 0} locations
                          </Typography>
                        </Box>
                        {manage && (
                          <Button
                            onClick={() => {
                              reset();
                              setMember({
                                ...m,
                                roleIds: [...m.roleIds],
                                locationIds: [...(m.locationIds ?? [])],
                                locationRoles:
                                  m.locationRoles?.map((scope) => ({
                                    locationId: scope.locationId,
                                    roleIds: [...scope.roleIds],
                                  })) ??
                                  (m.locationIds ?? []).map((locationId) => ({
                                    locationId,
                                    roleIds: [...m.roleIds],
                                  })),
                              });
                            }}
                          >
                            Manage access
                          </Button>
                        )}
                      </Stack>
                    </CardContent>
                  </Card>
                ))}
            </Stack>
          )}
          {tab === 'modules' && (
            <>
              <Alert severity="warning" sx={{ mb: 2 }}>
                Disabling a module blocks its API operations and removes its granted permissions.
                Data and role definitions are retained. Review connected workflows before disabling
                a module.
              </Alert>
              <Box
                sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: '1fr', md: 'repeat(2,minmax(0,1fr))' },
                  gap: 2,
                }}
              >
                {data.catalog
                  .filter((m) => m.name.toLowerCase().includes(search.toLowerCase()))
                  .map((m) => {
                    const state = data.modules.find((v) => v.key === m.key) ?? {
                      key: m.key,
                      enabled: true,
                      revision: 0,
                    };
                    return (
                      <Card key={m.key} variant="outlined">
                        <CardContent>
                          <Stack direction="row" alignItems="center" gap={2}>
                            <Box sx={{ flex: 1 }}>
                              <Typography fontWeight={700}>{m.name}</Typography>
                              <Typography variant="body2" color="text.secondary">
                                {m.permissions.length} permissions ·{' '}
                                {m.required
                                  ? 'Always enabled'
                                  : state.enabled
                                    ? 'Enabled'
                                    : 'Disabled'}
                              </Typography>
                            </Box>
                            <Switch
                              slotProps={{ input: { 'aria-label': `${m.name} enabled` } }}
                              checked={state.enabled}
                              disabled={!manage || m.required}
                              onChange={(_, enabled) => {
                                reset();
                                setModule({ ...state, enabled });
                              }}
                            />
                          </Stack>
                        </CardContent>
                      </Card>
                    );
                  })}
              </Box>
            </>
          )}
          {tab === 'audit' && (
            <Stack gap={1}>
              <Typography color="text.secondary">
                Latest 100 changes, with before and after values. Passwords are never recorded.
              </Typography>
              {data.audit
                .filter((a) =>
                  `${a.action} ${a.actor}`.toLowerCase().includes(search.toLowerCase()),
                )
                .map((a) => (
                  <Card key={a.id} variant="outlined">
                    <CardContent>
                      <Typography fontWeight={700}>
                        {a.action.replaceAll('.', ' ')} · {a.actor ?? 'System'}
                      </Typography>
                      <Typography color="text.secondary" variant="body2">
                        {new Date(a.at).toLocaleString()}
                      </Typography>
                      {a.after?.name != null && <Typography>{String(a.after.name)}</Typography>}
                      {a.after?.roleIds instanceof Array && (
                        <Typography>
                          Assigned roles: {roleNames(a.after.roleIds as string[])}
                        </Typography>
                      )}
                      {typeof a.after?.enabled === 'boolean' && (
                        <Typography>
                          {data.catalog.find((m) => m.key === a.targetId)?.name ?? a.targetId}:{' '}
                          {a.after.enabled ? 'enabled' : 'disabled'}
                        </Typography>
                      )}
                      {typeof a.after?.active === 'boolean' && (
                        <Typography>
                          Member access: {a.after.active ? 'active' : 'disabled'}
                        </Typography>
                      )}
                      {a.after?.permissions instanceof Array && (
                        <Typography variant="body2">
                          Granted:{' '}
                          {(a.after.permissions as string[])
                            .filter(
                              (p) => !(a.before?.permissions as string[] | undefined)?.includes(p),
                            )
                            .map((p) => labels.get(p) ?? p)
                            .join(', ') || 'None'}
                          <br />
                          Removed:{' '}
                          {((a.before?.permissions as string[] | undefined) ?? [])
                            .filter((p) => !(a.after?.permissions as string[])?.includes(p))
                            .map((p) => labels.get(p) ?? p)
                            .join(', ') || 'None'}
                        </Typography>
                      )}
                    </CardContent>
                  </Card>
                ))}
              {data.audit.length === 0 && (
                <Alert severity="info">No access changes have been recorded yet.</Alert>
              )}
            </Stack>
          )}
        </>
      )}
      <Dialog open={!!role} onClose={close} fullWidth maxWidth="md">
        <DialogTitle>
          {role?.id ? 'Edit' : role?.isTemplate ? 'Create template' : 'Create role'}
          {role?.id ? ` ${role.name}` : ''}
        </DialogTitle>
        <DialogContent>
          {role && data && (
            <GuidedForm
              busy={mutation.isPending}
              onCancel={close}
              enabled={manage}
              review={
                <Stack gap={1}>
                  <Typography fontWeight={700}>
                    {role.name} · {role.isTemplate ? 'Template' : 'Role'}
                  </Typography>
                  <Typography>{role.description}</Typography>
                  <Typography>{role.permissions.length} permissions</Typography>
                  {role.permissions.includes('access:manage') && (
                    <Alert severity="warning">
                      This role can change roles and grant access to other members.
                    </Alert>
                  )}
                  <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                    {role.permissions.map((p) => (
                      <Chip key={p} size="small" label={labels.get(p) ?? p} />
                    ))}
                  </Box>
                </Stack>
              }
              actions={
                manage ? (
                  <Button
                    variant="contained"
                    disabled={mutation.isPending}
                    onClick={() => mutation.mutate(() => saveRole(role))}
                  >
                    Save {role.isTemplate ? 'template' : 'role'}
                  </Button>
                ) : null
              }
            >
              <GuidedStep
                title="Role details"
                validate={() => (!role.name.trim() ? 'Enter a role or template name.' : undefined)}
              >
                <Stack gap={2}>
                  {!role.id && (
                    <TextField
                      select
                      label="Start from a template"
                      value={templateId}
                      onChange={(e) => {
                        setTemplateId(e.target.value);
                        const t = data.roles.find((r) => r.id === e.target.value);
                        if (t)
                          setRole({
                            ...role,
                            name: role.name.trim() ? role.name : t.name,
                            permissions: [...t.permissions],
                            description: t.description,
                          });
                      }}
                    >
                      <MenuItem value="">Choose a starting point</MenuItem>
                      {data.roles
                        .filter((r) => r.isTemplate)
                        .map((t) => (
                          <MenuItem key={t.id} value={t.id}>
                            {t.name}
                          </MenuItem>
                        ))}
                    </TextField>
                  )}
                  <TextField
                    required
                    label="Name"
                    value={role.name}
                    disabled={!manage}
                    inputProps={{ maxLength: 80 }}
                    onChange={(e) => setRole({ ...role, name: e.target.value })}
                  />
                  <TextField
                    label="Description"
                    multiline
                    minRows={2}
                    value={role.description}
                    disabled={!manage}
                    inputProps={{ maxLength: 1000 }}
                    onChange={(e) => setRole({ ...role, description: e.target.value })}
                  />
                </Stack>
              </GuidedStep>
              <GuidedStep
                title="Module permissions"
                description="Choose only the actions this responsibility needs. Counter sales also require shift, cash register, product, and player access."
              >
                <TextField
                  fullWidth
                  label="Find a module or permission"
                  value={permissionSearch}
                  onChange={(e) => setPermissionSearch(e.target.value)}
                  sx={{ mb: 2 }}
                />
                <Stack gap={2}>
                  {data.catalog
                    .filter((m) =>
                      `${m.name} ${m.permissions.map((p) => p.label).join(' ')}`
                        .toLowerCase()
                        .includes(permissionSearch.toLowerCase()),
                    )
                    .map((m) => {
                      const count = m.permissions.filter((p) =>
                        role.permissions.includes(p.key),
                      ).length;
                      return (
                        <Card key={m.key} variant="outlined">
                          <CardContent>
                            <FormControlLabel
                              label={
                                <Typography fontWeight={700}>
                                  {m.name} · {count}/{m.permissions.length}
                                </Typography>
                              }
                              control={
                                <Checkbox
                                  checked={count === m.permissions.length}
                                  indeterminate={count > 0 && count < m.permissions.length}
                                  disabled={!manage}
                                  onChange={(_, checked) =>
                                    setRole({
                                      ...role,
                                      permissions: checked
                                        ? [
                                            ...new Set([
                                              ...role.permissions,
                                              ...m.permissions.map((p) => p.key),
                                            ]),
                                          ]
                                        : role.permissions.filter(
                                            (p) => !m.permissions.some((v) => v.key === p),
                                          ),
                                    })
                                  }
                                />
                              }
                            />
                            <Box
                              sx={{
                                display: 'grid',
                                gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))' },
                              }}
                            >
                              {m.permissions.map((p) => (
                                <FormControlLabel
                                  key={p.key}
                                  label={
                                    <Box>
                                      <Typography variant="body2">{p.label}</Typography>
                                      {p.description && (
                                        <Typography variant="caption" color="text.secondary">
                                          {p.description}
                                        </Typography>
                                      )}
                                    </Box>
                                  }
                                  control={
                                    <Checkbox
                                      checked={role.permissions.includes(p.key)}
                                      disabled={!manage}
                                      onChange={(_, checked) =>
                                        setRole({
                                          ...role,
                                          permissions: checked
                                            ? normalizeGrants([...role.permissions, p.key])
                                            : role.permissions.filter(
                                                (v) =>
                                                  v !== p.key &&
                                                  (!p.key.endsWith(':read') ||
                                                    !v.startsWith(p.key.split(':')[0] + ':')),
                                              ),
                                        })
                                      }
                                    />
                                  }
                                />
                              ))}
                            </Box>
                          </CardContent>
                        </Card>
                      );
                    })}
                </Stack>
              </GuidedStep>
            </GuidedForm>
          )}
          {error}
        </DialogContent>
        {!manage && (
          <DialogActions>
            <Button onClick={close}>Close</Button>
          </DialogActions>
        )}
      </Dialog>
      <Dialog open={!!member || !!newMember} onClose={close} fullWidth maxWidth="sm">
        <DialogTitle>{member ? `Manage ${member.username}` : 'Add team member'}</DialogTitle>
        <DialogContent>
          {data && (member || newMember) && (
            <GuidedForm
              busy={mutation.isPending}
              onCancel={close}
              review={
                <Stack gap={1}>
                  <Typography>{member?.username ?? newMember?.username}</Typography>
                  <Typography>
                    {member?.active === false ? 'Access disabled' : 'Active access'}
                  </Typography>
                  <Typography>{roleNames(selectedIds)}</Typography>
                  <Typography>
                    {(member?.locationIds ?? newMember?.locationIds ?? []).length} locations
                  </Typography>
                  <Typography>
                    {grants.length} effective permissions after module restrictions
                  </Typography>
                  {newMember && <Typography>Password: ••••••••••••</Typography>}
                  {selectedIds.length === 0 && (
                    <Alert severity="warning">This member will have no module access.</Alert>
                  )}
                </Stack>
              }
              actions={
                <Button
                  variant="contained"
                  disabled={mutation.isPending}
                  onClick={() =>
                    mutation.mutate(() => (member ? saveMember(member) : createMember(newMember!)))
                  }
                >
                  Save member
                </Button>
              }
            >
              <GuidedStep
                title="Member details"
                validate={() =>
                  newMember &&
                  (!/^[a-zA-Z0-9_.-]{3,80}$/.test(newMember.username.trim()) ||
                    newMember.password.length < 12 ||
                    new TextEncoder().encode(newMember.password).length > 72)
                    ? 'Enter a username and a password of 12–72 bytes.'
                    : undefined
                }
              >
                <Stack gap={2}>
                  {newMember ? (
                    <>
                      <TextField
                        required
                        label="Username"
                        autoComplete="off"
                        value={newMember.username}
                        onChange={(e) => setNewMember({ ...newMember, username: e.target.value })}
                      />
                      <TextField
                        required
                        label="Initial password"
                        type="password"
                        autoComplete="new-password"
                        helperText="12–72 bytes. Share securely with this team member."
                        value={newMember.password}
                        onChange={(e) => setNewMember({ ...newMember, password: e.target.value })}
                      />
                    </>
                  ) : (
                    <>
                      <Typography>{member?.name || member?.username}</Typography>
                      <FormControlLabel
                        label="Allow access to this organization"
                        control={
                          <Switch
                            checked={member!.active}
                            onChange={(_, active) => setMember({ ...member!, active })}
                          />
                        }
                      />
                    </>
                  )}
                </Stack>
              </GuidedStep>
              <GuidedStep
                title="Assign roles"
                description="Choose roles to apply to all selected locations. You can then adjust each location below."
              >
                <Stack>
                  {data.roles
                    .filter((r) => !r.isTemplate)
                    .map((r) => (
                      <FormControlLabel
                        key={r.id}
                        label={
                          <Box>
                            <Typography>{r.name}</Typography>
                            <Typography variant="caption" color="text.secondary">
                              {r.permissions.length} permissions
                            </Typography>
                          </Box>
                        }
                        control={
                          <Checkbox
                            checked={selectedIds.includes(r.id)}
                            onChange={() => toggleRole(r.id)}
                          />
                        }
                      />
                    ))}
                </Stack>
              </GuidedStep>
              <GuidedStep
                title="Location access"
                description="Select every venue where this member may use their assigned roles."
                validate={() =>
                  (member?.locationIds ?? newMember?.locationIds ?? []).length === 0
                    ? 'Choose at least one location.'
                    : undefined
                }
              >
                {locations.error && <Alert severity="error">{locations.error.message}</Alert>}
                <Stack>
                  {locations.data?.map((location) => {
                    const selected = member?.locationIds ?? newMember?.locationIds ?? [];
                    return (
                      <FormControlLabel
                        key={location.id}
                        label={`${location.name} (${location.slug})`}
                        control={
                          <Checkbox
                            checked={selected.includes(location.id)}
                            onChange={(_, checked) => {
                              const locationIds = checked
                                ? [...selected, location.id]
                                : selected.filter((id) => id !== location.id);
                              if (member)
                                setMember({
                                  ...member,
                                  locationIds,
                                  locationRoles: checked
                                    ? [
                                        ...(member.locationRoles ?? []),
                                        { locationId: location.id, roleIds: [...member.roleIds] },
                                      ]
                                    : member.locationRoles?.filter(
                                        (scope) => scope.locationId !== location.id,
                                      ),
                                });
                              if (newMember)
                                setNewMember({
                                  ...newMember,
                                  locationIds,
                                  locationRoles: checked
                                    ? [
                                        ...newMember.locationRoles,
                                        {
                                          locationId: location.id,
                                          roleIds: [...newMember.roleIds],
                                        },
                                      ]
                                    : newMember.locationRoles.filter(
                                        (scope) => scope.locationId !== location.id,
                                      ),
                                });
                            }}
                          />
                        }
                      />
                    );
                  })}
                </Stack>
                {(member?.locationRoles ?? newMember?.locationRoles ?? []).map((scope) => (
                  <Card key={scope.locationId} variant="outlined" sx={{ mt: 1 }}>
                    <CardContent>
                      <Typography fontWeight={700}>
                        {locations.data?.find((location) => location.id === scope.locationId)
                          ?.name ?? scope.locationId}
                      </Typography>
                      {data.roles
                        .filter((role) => !role.isTemplate)
                        .map((role) => (
                          <FormControlLabel
                            key={role.id}
                            label={role.name}
                            control={
                              <Checkbox
                                checked={scope.roleIds.includes(role.id)}
                                onChange={(_, checked) => {
                                  const current = member ?? newMember!;
                                  const locationRoles = (current.locationRoles ?? []).map((item) =>
                                    item.locationId === scope.locationId
                                      ? {
                                          ...item,
                                          roleIds: checked
                                            ? [...item.roleIds, role.id]
                                            : item.roleIds.filter((id) => id !== role.id),
                                        }
                                      : item,
                                  );
                                  const roleIds = [
                                    ...new Set(locationRoles.flatMap((item) => item.roleIds)),
                                  ];
                                  if (member) setMember({ ...member, roleIds, locationRoles });
                                  if (newMember)
                                    setNewMember({ ...newMember, roleIds, locationRoles });
                                }}
                              />
                            }
                          />
                        ))}
                    </CardContent>
                  </Card>
                ))}
              </GuidedStep>
            </GuidedForm>
          )}
          {error}
        </DialogContent>
      </Dialog>
      <Dialog open={!!module || !!deleting} onClose={close} fullWidth>
        <DialogTitle>
          {module
            ? `${module.enabled ? 'Enable' : 'Disable'} ${data?.catalog.find((m) => m.key === module.key)?.name}`
            : `Delete ${deleting?.name}`}
        </DialogTitle>
        <DialogContent>
          <Alert severity="warning">
            {module
              ? 'This changes effective permissions for every affected role. Active sessions may require a new sign-in.'
              : 'This permanently removes the role or template. Assigned roles must be reassigned before deletion.'}
          </Alert>
          {error}
        </DialogContent>
        <DialogActions>
          <Button disabled={mutation.isPending} onClick={close}>
            Cancel
          </Button>
          <Button
            variant="contained"
            color={deleting || module?.enabled === false ? 'error' : 'primary'}
            disabled={mutation.isPending}
            onClick={() =>
              mutation.mutate(() => (module ? saveModule(module) : deleteRole(deleting!)))
            }
          >
            Confirm change
          </Button>
        </DialogActions>
      </Dialog>
    </PageShell>
  );
}
