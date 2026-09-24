import { local, toastUtils } from '@gaming-cafe/utils';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Button,
  Card,
  CardContent,
  CardHeader,
  CircularProgress,
  Divider,
  FormControl,
  Grid,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { decodeJwtPayload } from '../../../lib/authSession';
import {
  deleteSettingOverride,
  getEffectiveSettings,
  getSettingCatalog,
  getSettingHistory,
  getVenueLocations,
  putSettingOverride,
  type ResolvedSetting,
  type SettingDefinition,
} from '../../../services/config';
import PricingRulesPanel from './PricingRulesPanel';

const DEFAULT_ORGANIZATION_ID = '00000000-0000-4000-8000-000000000001';

function currentOrganizationId(): string {
  const token = local.get<string>('accessToken');
  if (!token) return DEFAULT_ORGANIZATION_ID;
  const claims = decodeJwtPayload(token);
  return claims?.tenantId ?? claims?.orgIds?.[0] ?? DEFAULT_ORGANIZATION_ID;
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined) return '';
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

function parseValue(definition: SettingDefinition, raw: string): unknown {
  switch (definition.valueType) {
    case 'number': {
      const value = Number.parseFloat(raw);
      if (!Number.isFinite(value)) throw new Error(`${definition.key} must be a number`);
      return value;
    }
    case 'integer': {
      const value = Number.parseInt(raw, 10);
      if (!Number.isInteger(value)) throw new Error(`${definition.key} must be an integer`);
      return value;
    }
    case 'boolean':
      return raw === 'true';
    case 'uuid':
      return raw.trim() || null;
    default:
      return raw.trim();
  }
}

function settingLabel(key: string): string {
  return (key.split('.').at(-1) ?? key)
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function ScopedSettingField({
  definition,
  resolved,
  rawValue,
  targetScope,
  saving,
  onChange,
  onSave,
  onRevert,
}: {
  definition: SettingDefinition;
  resolved?: ResolvedSetting;
  rawValue: string;
  targetScope: 'organization' | 'location';
  saving: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
  onRevert: () => void;
}) {
  const exactOverride = resolved?.sourceScope === targetScope;
  const helper = `${definition.description} · Effective from ${resolved?.sourceScope ?? 'platform'}${
    resolved?.revision ? ` · revision ${resolved.revision}` : ''
  }`;

  return (
    <Box sx={{ display: 'flex', gap: 1, alignItems: 'flex-start' }}>
      {definition.valueType === 'boolean' ? (
        <FormControl size="small" fullWidth>
          <InputLabel>{settingLabel(definition.key)}</InputLabel>
          <Select
            label={settingLabel(definition.key)}
            value={rawValue || 'false'}
            onChange={(event) => onChange(event.target.value)}
          >
            <MenuItem value="true">Enabled</MenuItem>
            <MenuItem value="false">Disabled</MenuItem>
          </Select>
          <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5 }}>
            {helper}
          </Typography>
        </FormControl>
      ) : (
        <TextField
          label={settingLabel(definition.key)}
          size="small"
          fullWidth
          value={rawValue}
          type={
            definition.valueType === 'number' || definition.valueType === 'integer'
              ? 'number'
              : 'text'
          }
          helperText={helper}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
      <Stack spacing={0.5}>
        <Button variant="outlined" size="small" disabled={saving} onClick={onSave}>
          {saving ? <CircularProgress size={18} /> : 'Save'}
        </Button>
        <Button size="small" color="inherit" disabled={saving || !exactOverride} onClick={onRevert}>
          Revert
        </Button>
      </Stack>
    </Box>
  );
}

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const organizationId = useMemo(currentOrganizationId, []);
  const [locationId, setLocationId] = useState('');
  const [reason, setReason] = useState('Venue settings update');
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState<string | null>(null);

  const catalogQuery = useQuery({
    queryKey: ['settings-catalog', organizationId],
    queryFn: () => getSettingCatalog(organizationId),
  });
  const locationsQuery = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
  });
  const effectiveQuery = useQuery({
    queryKey: ['effective-settings', organizationId, locationId],
    queryFn: () => getEffectiveSettings(organizationId, locationId || undefined),
  });
  const historyQuery = useQuery({
    queryKey: ['settings-history', organizationId, locationId],
    queryFn: () => getSettingHistory(organizationId, locationId || undefined),
  });

  useEffect(() => {
    if (!effectiveQuery.data) return;
    setValues(
      Object.fromEntries(
        effectiveQuery.data.map((setting) => [setting.key, displayValue(setting.value)]),
      ),
    );
  }, [effectiveQuery.data]);

  const resolvedByKey = useMemo(
    () => new Map((effectiveQuery.data ?? []).map((setting) => [setting.key, setting])),
    [effectiveQuery.data],
  );
  const groups = useMemo(() => {
    const grouped = new Map<string, SettingDefinition[]>();
    for (const definition of catalogQuery.data ?? []) {
      const list = grouped.get(definition.category) ?? [];
      list.push(definition);
      grouped.set(definition.category, list);
    }
    return [...grouped.entries()];
  }, [catalogQuery.data]);

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ['effective-settings', organizationId] }),
      queryClient.invalidateQueries({ queryKey: ['settings-history', organizationId] }),
    ]);
  };

  const handleSave = async (definition: SettingDefinition) => {
    if (reason.trim().length < 3) {
      toastUtils.error('Enter a change reason of at least 3 characters');
      return;
    }
    const resolved = resolvedByKey.get(definition.key);
    const targetScope = locationId ? 'location' : 'organization';
    const expectedRevision = resolved?.sourceScope === targetScope ? resolved.revision : 0;
    setSaving(definition.key);
    try {
      await putSettingOverride(organizationId, definition.key, {
        locationId: locationId || undefined,
        value: parseValue(definition, values[definition.key] ?? ''),
        reason: reason.trim(),
        expectedRevision,
      });
      await refresh();
      toastUtils.success(`${definition.key} saved`);
    } catch (error) {
      toastUtils.error(error instanceof Error ? error.message : `Failed to save ${definition.key}`);
    } finally {
      setSaving(null);
    }
  };

  const handleRevert = async (definition: SettingDefinition) => {
    const resolved = resolvedByKey.get(definition.key);
    if (!resolved) return;
    setSaving(definition.key);
    try {
      await deleteSettingOverride(organizationId, definition.key, {
        locationId: locationId || undefined,
        expectedRevision: resolved.revision,
        reason: reason.trim(),
      });
      await refresh();
      toastUtils.success(`${definition.key} reverted to inherited value`);
    } catch (error) {
      toastUtils.error(
        error instanceof Error ? error.message : `Failed to revert ${definition.key}`,
      );
    } finally {
      setSaving(null);
    }
  };

  const handleRollbackRevision = async (revision: {
    id: number;
    key: string;
    operation: 'create' | 'update' | 'delete';
    oldValue?: unknown;
  }) => {
    const resolved = resolvedByKey.get(revision.key);
    const targetScope = locationId ? 'location' : 'organization';
    const expectedRevision = resolved?.sourceScope === targetScope ? resolved.revision : 0;
    const rollbackReason = `Rollback configuration revision ${revision.id}`;
    setSaving(revision.key);
    try {
      if (revision.operation === 'create') {
        await deleteSettingOverride(organizationId, revision.key, {
          locationId: locationId || undefined,
          expectedRevision,
          reason: rollbackReason,
        });
      } else {
        await putSettingOverride(organizationId, revision.key, {
          locationId: locationId || undefined,
          value: revision.oldValue,
          expectedRevision,
          reason: rollbackReason,
        });
      }
      await refresh();
      toastUtils.success(`${revision.key} rolled back`);
    } catch (error) {
      toastUtils.error(error instanceof Error ? error.message : 'Configuration rollback failed');
    } finally {
      setSaving(null);
    }
  };

  const isLoading = catalogQuery.isLoading || effectiveQuery.isLoading || locationsQuery.isLoading;
  if (isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 10 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box sx={{ py: { xs: 3, md: 4 }, px: { xs: 2, sm: 3, md: 4 } }}>
      <Typography variant="h3" fontWeight={700} gutterBottom>
        Configuration and policies
      </Typography>
      <Typography variant="body1" color="text.secondary" sx={{ mb: 3 }}>
        Typed, scoped and auditable venue behavior. Location values override organization values.
      </Typography>

      <Stack direction={{ xs: 'column', md: 'row' }} spacing={2} sx={{ mb: 3 }}>
        <FormControl size="small" sx={{ minWidth: 260 }}>
          <InputLabel>Configuration scope</InputLabel>
          <Select
            label="Configuration scope"
            value={locationId}
            onChange={(event) => setLocationId(event.target.value)}
          >
            <MenuItem value="">Organization defaults</MenuItem>
            {(locationsQuery.data ?? []).map((location) => (
              <MenuItem key={location.id} value={location.id}>
                {location.name}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <TextField
          size="small"
          label="Change reason"
          value={reason}
          onChange={(event) => setReason(event.target.value)}
          fullWidth
          helperText="Stored with every configuration revision"
        />
      </Stack>

      <Grid container spacing={3}>
        {groups.map(([category, definitions]) => (
          <Grid key={category} size={{ xs: 12, lg: 6 }}>
            <Card variant="outlined">
              <CardHeader
                title={category.replaceAll('_', ' ')}
                sx={{ textTransform: 'capitalize' }}
              />
              <Divider />
              <CardContent>
                <Stack spacing={2}>
                  {definitions.map((definition) => (
                    <ScopedSettingField
                      key={definition.key}
                      definition={definition}
                      resolved={resolvedByKey.get(definition.key)}
                      rawValue={values[definition.key] ?? ''}
                      targetScope={locationId ? 'location' : 'organization'}
                      saving={saving === definition.key}
                      onChange={(value) =>
                        setValues((current) => ({ ...current, [definition.key]: value }))
                      }
                      onSave={() => handleSave(definition)}
                      onRevert={() => handleRevert(definition)}
                    />
                  ))}
                </Stack>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>

      <Box sx={{ mt: 3 }}>
        <PricingRulesPanel organizationId={organizationId} locationId={locationId || undefined} />
      </Box>

      <Accordion sx={{ mt: 3 }}>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography fontWeight={700}>Configuration history</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1}>
            {(historyQuery.data ?? []).map((revision) => (
              <Box key={revision.id} sx={{ borderBottom: 1, borderColor: 'divider', pb: 1 }}>
                <Typography variant="body2" fontWeight={600}>
                  {revision.key} · {revision.operation} · revision {revision.revision}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {revision.reason} · {new Date(revision.createdAt).toLocaleString()}
                </Typography>
                <Typography
                  variant="caption"
                  component="pre"
                  sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', my: 0.5 }}
                >
                  {JSON.stringify(revision.oldValue)} → {JSON.stringify(revision.newValue)}
                </Typography>
                <Button
                  size="small"
                  disabled={saving === revision.key}
                  onClick={() => handleRollbackRevision(revision)}
                >
                  Roll back this change
                </Button>
              </Box>
            ))}
            {!historyQuery.data?.length && (
              <Typography color="text.secondary">No changes recorded for this scope.</Typography>
            )}
          </Stack>
        </AccordionDetails>
      </Accordion>
    </Box>
  );
}
