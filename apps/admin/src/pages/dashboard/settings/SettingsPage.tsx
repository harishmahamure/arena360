import { PageHeader, PageShell, WizardProgress } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import {
  BusinessOutlined,
  CheckCircleOutline,
  HistoryOutlined,
  Search,
  TuneOutlined,
} from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Card,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControl,
  InputAdornment,
  InputLabel,
  List,
  ListItemButton,
  ListItemText,
  MenuItem,
  Select,
  Skeleton,
  Stack,
  Switch,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { alpha } from '@mui/material/styles';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { decodeJwtPayload } from '../../../lib/authSession';
import {
  deleteSettingOverride,
  getEffectiveSettings,
  getSettingCatalog,
  getSettingHistory,
  getVenueLocations,
  putSettingOverride,
  type SettingDefinition,
  type SettingRevision,
} from '../../../services/config';
import PricingRulesPanel from './PricingRulesPanel';
import { displayValue, parseSettingValue, settingLabel } from './settingValues';

// Legacy deployments use this organization before tenant-aware tokens are issued.
const DEFAULT_ORGANIZATION_ID = '00000000-0000-4000-8000-000000000001';
function currentOrganizationId(): string {
  const token = local.get<string>('accessToken');
  const claims = token ? decodeJwtPayload(token) : null;
  return claims?.tenantId ?? claims?.orgIds?.[0] ?? DEFAULT_ORGANIZATION_ID;
}
function settingChoices(kind: 'currency' | 'timezone'): string[] {
  const supportedValues = (Intl as unknown as { supportedValuesOf?: (key: string) => string[] })
    .supportedValuesOf;
  try {
    return supportedValues?.(kind === 'timezone' ? 'timeZone' : 'currency') ?? [];
  } catch {
    return kind === 'currency' ? ['INR', 'USD', 'EUR', 'GBP'] : ['UTC', 'Asia/Kolkata'];
  }
}
const CURRENCY_CHOICES = settingChoices('currency');
const TIMEZONE_CHOICES = settingChoices('timezone');
function groupSettingSteps(definitions: SettingDefinition[]): SettingDefinition[][] {
  return [...new Set(definitions.map((item) => item.category))].flatMap((category) => {
    const group = definitions.filter((item) => item.category === category);
    return Array.from({ length: Math.ceil(group.length / 4) }, (_, index) =>
      group.slice(index * 4, index * 4 + 4),
    );
  });
}
type Draft = { raw: string; expectedRevision: number };
type Confirmation =
  | { type: 'save' }
  | { type: 'revert'; key: string }
  | { type: 'rollback'; revision: SettingRevision }
  | { type: 'scope'; locationId: string }
  | { type: 'discard' };

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const { can } = usePermissions();
  const canWrite = can(Permission.SettingsWrite);
  const organizationId = useMemo(currentOrganizationId, []);
  const [locationId, setLocationId] = useState('');
  const [tab, setTab] = useState(0);
  const [pricingDirty, setPricingDirty] = useState(false);
  const [category, setCategory] = useState('all');
  const [search, setSearch] = useState('');
  const [settingsStep, setSettingsStep] = useState(0);
  const [reason, setReason] = useState('');
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const targetScope = locationId ? 'location' : 'organization';
  const catalog = useQuery({
    queryKey: ['settings-catalog', organizationId],
    queryFn: () => getSettingCatalog(organizationId),
  });
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId],
    queryFn: () => getVenueLocations(organizationId),
  });
  const effective = useQuery({
    queryKey: ['effective-settings', organizationId, locationId],
    queryFn: () => getEffectiveSettings(organizationId, locationId || undefined),
  });
  const history = useQuery({
    queryKey: ['settings-history', organizationId, locationId],
    queryFn: () => getSettingHistory(organizationId, locationId || undefined),
  });
  const resolved = new Map((effective.data ?? []).map((item) => [item.key, item]));
  const categories = [...new Set((catalog.data ?? []).map((item) => item.category))];
  const definitions = (catalog.data ?? []).filter(
    (item) =>
      (category === 'all' || item.category === category) &&
      `${item.key} ${item.description}`.toLowerCase().includes(search.toLowerCase()),
  );
  const groupedSettings = groupSettingSteps(definitions);
  const settingSteps = groupedSettings.length ? groupedSettings : [[]];
  const currentSettingsStep = Math.min(settingsStep, settingSteps.length - 1);
  const visibleDefinitions = settingSteps[currentSettingsStep] ?? [];
  const advanceSettings = () => {
    const nextErrors: Record<string, string> = {};
    for (const definition of visibleDefinitions) {
      const draft = drafts[definition.key];
      if (!draft) continue;
      try {
        parseSettingValue(definition, draft.raw);
      } catch (error) {
        nextErrors[definition.key] = error instanceof Error ? error.message : 'Invalid value';
      }
    }
    setErrors((previous) => ({ ...previous, ...nextErrors }));
    if (!Object.keys(nextErrors).length) setSettingsStep(currentSettingsStep + 1);
  };
  const draftKeys = Object.keys(drafts);
  const scopeLabel =
    locations.data?.find((item) => item.id === locationId)?.name ?? 'Organization defaults';
  const scopeHistory = (history.data ?? []).filter(
    (item) => (item.locationId ?? '') === locationId,
  );

  useEffect(() => {
    if (!Object.keys(drafts).length && !pricingDirty) return;
    const beforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };
    const beforeNavigation = (event: MouseEvent) => {
      const anchor = (event.target as Element).closest?.('a[href]');
      if (
        anchor &&
        anchor.getAttribute('href') !== '#' &&
        !anchor.getAttribute('href')?.startsWith('#') &&
        !window.confirm('You have unsaved configuration changes. Leave without saving?')
      ) {
        event.preventDefault();
        event.stopPropagation();
      }
    };
    window.addEventListener('beforeunload', beforeUnload);
    document.addEventListener('click', beforeNavigation, true);
    return () => {
      window.removeEventListener('beforeunload', beforeUnload);
      document.removeEventListener('click', beforeNavigation, true);
    };
  }, [drafts, pricingDirty]);

  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ['effective-settings', organizationId] }),
      queryClient.invalidateQueries({ queryKey: ['settings-history', organizationId] }),
    ]);
  const edit = (definition: SettingDefinition, raw: string) => {
    const current = resolved.get(definition.key);
    setDrafts((previous) => {
      const next = { ...previous };
      if (raw === displayValue(current?.value ?? definition.defaultValue))
        delete next[definition.key];
      else
        next[definition.key] = {
          raw,
          expectedRevision:
            previous[definition.key]?.expectedRevision ??
            (current?.sourceScope === targetScope ? current.revision : 0),
        };
      return next;
    });
    setErrors((previous) => ({ ...previous, [definition.key]: '' }));
  };
  const review = () => {
    const nextErrors: Record<string, string> = {};
    for (const key of draftKeys) {
      const definition = catalog.data?.find((item) => item.key === key);
      if (!definition) continue;
      try {
        parseSettingValue(definition, drafts[key]?.raw ?? '');
      } catch (error) {
        nextErrors[key] = error instanceof Error ? error.message : 'Invalid value';
      }
    }
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length) {
      toastUtils.error('Check the highlighted settings before saving.');
      setTab(0);
      setCategory('all');
      setSearch('');
      const firstInvalid = groupSettingSteps(catalog.data ?? []).findIndex((step) =>
        step.some((definition) => nextErrors[definition.key]),
      );
      setSettingsStep(Math.max(0, firstInvalid));
      return;
    }
    setConfirmation({ type: 'save' });
  };
  const confirm = async () => {
    if (!confirmation) return;
    if (confirmation.type === 'discard' || confirmation.type === 'scope') {
      if (confirmation.type === 'scope') {
        setLocationId(confirmation.locationId);
        setPricingDirty(false);
      }
      setDrafts({});
      setErrors({});
      setConfirmation(null);
      return;
    }
    if (!canWrite || reason.trim().length < 3) return;
    setSaving(true);
    try {
      if (confirmation.type === 'save') {
        // Each endpoint is revision-checked. Keep unsaved drafts if a later request fails.
        for (const key of draftKeys) {
          const definition = catalog.data?.find((item) => item.key === key);
          if (!definition) continue;
          await putSettingOverride(organizationId, key, {
            locationId: locationId || undefined,
            value: parseSettingValue(definition, drafts[key]?.raw ?? ''),
            expectedRevision: drafts[key]?.expectedRevision ?? 0,
            reason: reason.trim(),
          });
          setDrafts((previous) => {
            const next = { ...previous };
            delete next[key];
            return next;
          });
        }
        toastUtils.success('Configuration changes saved.');
      } else {
        const key = confirmation.type === 'revert' ? confirmation.key : confirmation.revision.key;
        const current = resolved.get(key);
        const expectedRevision = current?.sourceScope === targetScope ? current.revision : 0;
        if (confirmation.type === 'revert' || confirmation.revision.operation === 'create') {
          await deleteSettingOverride(organizationId, key, {
            locationId: locationId || undefined,
            expectedRevision,
            reason: reason.trim(),
          });
        } else {
          await putSettingOverride(organizationId, key, {
            locationId: locationId || undefined,
            value: confirmation.revision.oldValue,
            expectedRevision,
            reason: reason.trim(),
          });
        }
        toastUtils.success('Configuration updated.');
      }
      setConfirmation(null);
      setReason('');
    } catch (error) {
      toastUtils.error(
        error instanceof Error
          ? error.message
          : 'Could not save changes. Unsaved edits are retained.',
      );
      setConfirmation(null);
    } finally {
      await refresh();
      setSaving(false);
    }
  };
  const isLoading = catalog.isLoading || effective.isLoading || locations.isLoading;
  const hasError = catalog.isError || effective.isError || locations.isError;

  return (
    <PageShell
      header={
        <PageHeader
          title="Configuration"
          description="One place to manage how your organization and locations operate."
        />
      }
    >
      <Card sx={{ mb: 3, p: 2.5 }}>
        <Stack direction={{ xs: 'column', md: 'row' }} spacing={2} alignItems={{ md: 'center' }}>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, flex: 1 }}>
            <Box
              sx={{
                color: 'primary.main',
                bgcolor: 'primary.light',
                p: 1.2,
                borderRadius: 2,
                display: 'flex',
              }}
            >
              <BusinessOutlined />
            </Box>
            <Box>
              <Typography fontWeight={650}>Configuration scope</Typography>
              <Typography variant="body2" color="text.secondary">
                Location overrides take priority over organization defaults.
              </Typography>
            </Box>
          </Box>
          <FormControl size="small" sx={{ minWidth: { xs: 0, md: 250 } }}>
            <InputLabel id="scope-label" shrink>
              Editing scope
            </InputLabel>
            <Select
              labelId="scope-label"
              displayEmpty
              renderValue={() => scopeLabel}
              label="Editing scope"
              value={locationId}
              disabled={saving || locations.isLoading || locations.isError}
              onChange={(event) => {
                if (draftKeys.length || pricingDirty)
                  setConfirmation({ type: 'scope', locationId: event.target.value });
                else {
                  setLocationId(event.target.value);
                  setErrors({});
                }
              }}
            >
              <MenuItem value="">Organization defaults</MenuItem>
              {(locations.data ?? []).map((item) => (
                <MenuItem key={item.id} value={item.id}>
                  {item.name}
                  {!item.isActive ? ' (inactive)' : ''}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
        </Stack>
      </Card>
      {!canWrite && (
        <Alert severity="info" sx={{ mb: 2 }}>
          You have view-only access to configuration. Contact an administrator to make changes.
        </Alert>
      )}
      <Tabs
        value={tab}
        onChange={(_, next) => setTab(next)}
        aria-label="Configuration sections"
        sx={{ mb: 3, borderBottom: 1, borderColor: 'divider' }}
        variant="scrollable"
        allowScrollButtonsMobile
      >
        <Tab
          label="General settings"
          icon={<TuneOutlined sx={{ fontSize: 18 }} />}
          iconPosition="start"
        />
        <Tab
          label="Pricing policies"
          icon={<CheckCircleOutline sx={{ fontSize: 18 }} />}
          iconPosition="start"
        />
        <Tab
          label="Change history"
          icon={<HistoryOutlined sx={{ fontSize: 18 }} />}
          iconPosition="start"
        />
      </Tabs>
      {hasError ? (
        <Alert
          severity="error"
          action={
            <Button
              onClick={() => {
                void catalog.refetch();
                void effective.refetch();
                void locations.refetch();
              }}
            >
              Retry
            </Button>
          }
        >
          Configuration could not be loaded. Check your connection and try again.
        </Alert>
      ) : isLoading ? (
        <Stack spacing={2} aria-label="Loading configuration">
          <Skeleton variant="rounded" height={80} />
          <Skeleton variant="rounded" height={300} />
        </Stack>
      ) : (
        <>
          {tab === 0 && (
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: { xs: '1fr', lg: '200px minmax(0, 1fr)' },
                gap: 3,
              }}
            >
              <Box>
                <Typography variant="overline" color="text.secondary">
                  SETTINGS DIRECTORY
                </Typography>
                <TextField
                  select
                  fullWidth
                  label="Settings category"
                  value={category}
                  onChange={(event) => {
                    setCategory(event.target.value);
                    setSettingsStep(0);
                  }}
                  sx={{ display: { xs: 'block', lg: 'none' }, mt: 1 }}
                >
                  {['all', ...categories].map((item) => (
                    <MenuItem key={item} value={item}>
                      {item === 'all' ? 'All settings' : settingLabel(item)}
                    </MenuItem>
                  ))}
                </TextField>
                <List
                  aria-label="Settings categories"
                  sx={{ display: { xs: 'none', lg: 'block' }, gap: 0.5 }}
                >
                  {['all', ...categories].map((item) => (
                    <ListItemButton
                      key={item}
                      selected={category === item}
                      onClick={() => {
                        setCategory(item);
                        setSettingsStep(0);
                      }}
                      sx={{ borderRadius: 1.5, mb: 0.5, py: 0.6 }}
                    >
                      <ListItemText
                        primary={item === 'all' ? 'All settings' : settingLabel(item)}
                        primaryTypographyProps={{
                          fontSize: 12,
                          fontWeight: category === item ? 650 : 450,
                        }}
                      />
                      <Typography variant="caption" color="text.secondary" sx={{ ml: 2 }}>
                        {item === 'all'
                          ? catalog.data?.length
                          : catalog.data?.filter((def) => def.category === item).length}
                      </Typography>
                    </ListItemButton>
                  ))}
                </List>
                <Alert
                  icon={false}
                  severity="info"
                  sx={{ mt: 2, display: { xs: 'none', lg: 'block' }, fontSize: 11 }}
                >
                  Platform → Organization → Location
                  <br />
                  <br />
                  Inherited values stay in sync until you create an override.
                </Alert>
              </Box>
              <Box sx={{ minWidth: 0 }}>
                <WizardProgress
                  titles={settingSteps.map((items, index) =>
                    items.length
                      ? `${settingLabel(items[0]?.category ?? 'Settings')} · ${index + 1}`
                      : 'Settings',
                  )}
                  activeStep={currentSettingsStep}
                  disabled={saving}
                  onBackTo={setSettingsStep}
                />
                <TextField
                  fullWidth
                  label="Search settings"
                  placeholder="Search by name, key, or description"
                  value={search}
                  onChange={(event) => {
                    setSearch(event.target.value);
                    setSettingsStep(0);
                  }}
                  slotProps={{
                    input: {
                      startAdornment: (
                        <InputAdornment position="start">
                          <Search fontSize="small" />
                        </InputAdornment>
                      ),
                    },
                  }}
                  sx={{ mb: 2 }}
                />
                <Card>
                  <Box
                    sx={{
                      p: 2.5,
                      borderBottom: 1,
                      borderColor: 'divider',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      gap: 2,
                    }}
                  >
                    <Box>
                      <Typography variant="h6">
                        {category === 'all' ? 'All settings' : settingLabel(category)}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {definitions.length} settings · {scopeLabel}
                      </Typography>
                    </Box>
                    <Chip
                      variant="outlined"
                      label={locationId ? 'Location scope' : 'Organization scope'}
                    />
                  </Box>
                  {visibleDefinitions.map((definition) => {
                    const value = resolved.get(definition.key);
                    const raw =
                      drafts[definition.key]?.raw ??
                      displayValue(value?.value ?? definition.defaultValue);
                    const allowed = definition.allowedScopes.includes(targetScope);
                    const editable = canWrite && allowed && !saving;
                    return (
                      <Box
                        key={definition.key}
                        sx={{
                          p: { xs: 2, sm: 2.5 },
                          borderBottom: 1,
                          borderColor: 'divider',
                          '&:last-child': { borderBottom: 0 },
                          bgcolor: drafts[definition.key]
                            ? (theme) =>
                                alpha(
                                  theme.palette.primary.main,
                                  theme.palette.mode === 'dark' ? 0.12 : 0.05,
                                )
                            : undefined,
                          borderLeft: 3,
                          borderLeftColor: drafts[definition.key] ? 'primary.main' : 'transparent',
                        }}
                      >
                        <Box
                          sx={{
                            display: 'grid',
                            gridTemplateColumns: {
                              xs: '1fr',
                              md: 'minmax(0, 1fr) minmax(180px, 38%)',
                            },
                            gap: 2.5,
                            alignItems: 'center',
                          }}
                        >
                          <Box>
                            <Typography component="div" fontWeight={600} fontSize={13}>
                              {settingLabel(definition.key)}{' '}
                              {drafts[definition.key] && (
                                <Chip label="Unsaved" size="small" color="warning" sx={{ ml: 1 }} />
                              )}
                            </Typography>
                            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.4 }}>
                              {definition.description}
                            </Typography>
                            <Typography
                              variant="caption"
                              color="text.secondary"
                              sx={{ fontFamily: 'monospace', fontSize: 10 }}
                            >
                              {definition.key}
                            </Typography>
                          </Box>
                          {definition.valueType === 'boolean' ? (
                            <Box
                              sx={{
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'flex-end',
                              }}
                            >
                              <Typography variant="body2">
                                {raw === 'true' ? 'Enabled' : 'Disabled'}
                              </Typography>
                              <Switch
                                checked={raw === 'true'}
                                disabled={!editable}
                                onChange={(_, checked) => edit(definition, String(checked))}
                                slotProps={{
                                  input: { 'aria-label': settingLabel(definition.key) },
                                }}
                              />
                            </Box>
                          ) : definition.valueType === 'currency' ||
                            definition.valueType === 'timezone' ? (
                            <Autocomplete
                              freeSolo
                              options={
                                definition.valueType === 'currency'
                                  ? CURRENCY_CHOICES
                                  : TIMEZONE_CHOICES
                              }
                              value={raw}
                              disabled={!editable}
                              onChange={(_, value) => edit(definition, value ?? '')}
                              onInputChange={(_, value, reason) => {
                                if (reason === 'input' || reason === 'clear')
                                  edit(definition, value);
                              }}
                              renderInput={(params) => (
                                <TextField
                                  {...params}
                                  label={settingLabel(definition.key)}
                                  error={!!errors[definition.key]}
                                  helperText={
                                    errors[definition.key] ||
                                    (definition.valueType === 'currency'
                                      ? 'Search a currency code, such as INR.'
                                      : 'Search a timezone, such as Asia/Kolkata.')
                                  }
                                />
                              )}
                            />
                          ) : (
                            <TextField
                              fullWidth
                              label={settingLabel(definition.key)}
                              value={raw}
                              disabled={!editable}
                              error={!!errors[definition.key]}
                              helperText={errors[definition.key]}
                              type={
                                definition.sensitive
                                  ? 'password'
                                  : definition.valueType === 'integer' ||
                                      definition.valueType === 'number'
                                    ? 'number'
                                    : definition.validation.format === 'HH:MM'
                                      ? 'time'
                                      : 'text'
                              }
                              onChange={(event) => edit(definition, event.target.value)}
                              slotProps={{
                                htmlInput: {
                                  step: definition.valueType === 'integer' ? 1 : 'any',
                                  min: definition.validation.minimum,
                                  max: definition.validation.maximum,
                                },
                              }}
                            />
                          )}
                        </Box>
                        <Stack
                          direction="row"
                          alignItems="center"
                          spacing={1}
                          sx={{ mt: 1.5, flexWrap: 'wrap', rowGap: 1 }}
                        >
                          <Chip
                            size="small"
                            variant="outlined"
                            label={
                              value?.sourceScope === targetScope
                                ? 'Overridden here'
                                : `Inherited from ${value?.sourceScope ?? 'platform'}`
                            }
                            sx={{ fontWeight: 450 }}
                          />
                          {!allowed && (
                            <Typography variant="caption" color="text.secondary">
                              Managed at organization level
                            </Typography>
                          )}
                          {value?.sourceScope === targetScope && editable && (
                            <Button
                              size="small"
                              disabled={draftKeys.length > 0}
                              onClick={() =>
                                setConfirmation({ type: 'revert', key: definition.key })
                              }
                              sx={{ minHeight: 24, fontSize: 11 }}
                            >
                              Restore inherited value
                            </Button>
                          )}
                        </Stack>
                      </Box>
                    );
                  })}
                  {definitions.length === 0 && (
                    <Box sx={{ p: 6, textAlign: 'center' }}>
                      <Typography fontWeight={600}>No matching settings</Typography>
                      <Typography color="text.secondary" variant="body2">
                        Try a different search or category.
                      </Typography>
                      <Button
                        onClick={() => {
                          setSearch('');
                          setCategory('all');
                          setSettingsStep(0);
                        }}
                      >
                        Clear filters
                      </Button>
                    </Box>
                  )}
                </Card>
                <Stack direction="row" justifyContent="space-between" spacing={2} sx={{ mt: 2 }}>
                  <Button
                    disabled={saving || currentSettingsStep === 0}
                    onClick={() => setSettingsStep(currentSettingsStep - 1)}
                  >
                    Back
                  </Button>
                  {currentSettingsStep < settingSteps.length - 1 ? (
                    <Button variant="contained" disabled={saving} onClick={advanceSettings}>
                      Continue
                    </Button>
                  ) : (
                    <Button
                      variant="contained"
                      disabled={saving || !draftKeys.length}
                      onClick={review}
                    >
                      Review changes
                    </Button>
                  )}
                </Stack>
              </Box>
            </Box>
          )}
          <Box sx={{ display: tab === 1 ? 'block' : 'none' }}>
            <PricingRulesPanel
              key={locationId}
              organizationId={organizationId}
              locationId={locationId || undefined}
              readOnly={!can(Permission.RulesEdit)}
              onDirtyChange={setPricingDirty}
            />
          </Box>
          {tab === 2 && (
            <Card sx={{ p: 3 }}>
              <Typography variant="h6" sx={{ mb: 0.5 }}>
                Change history
              </Typography>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
                Recent changes to {scopeLabel.toLowerCase()}. Restoring a change creates a new
                revision.
              </Typography>
              {history.isLoading ? (
                <Skeleton height={100} />
              ) : history.isError ? (
                <Alert
                  severity="error"
                  action={<Button onClick={() => void history.refetch()}>Retry</Button>}
                >
                  Change history could not be loaded.
                </Alert>
              ) : scopeHistory.length ? (
                scopeHistory.map((revision) => (
                  <Box key={revision.id} sx={{ borderTop: 1, borderColor: 'divider', py: 2.5 }}>
                    <Stack
                      direction={{ xs: 'column', sm: 'row' }}
                      justifyContent="space-between"
                      spacing={1}
                    >
                      <Box>
                        <Typography component="div" fontWeight={600}>
                          {settingLabel(revision.key)}{' '}
                          <Chip label={revision.operation} sx={{ ml: 1 }} />
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          {new Date(revision.createdAt).toLocaleString()} · Revision{' '}
                          {revision.revision}
                        </Typography>
                      </Box>
                      {canWrite && (
                        <Button
                          disabled={
                            saving ||
                            draftKeys.length > 0 ||
                            !catalog.data
                              ?.find((def) => def.key === revision.key)
                              ?.allowedScopes.includes(targetScope)
                          }
                          onClick={() => setConfirmation({ type: 'rollback', revision })}
                          size="small"
                        >
                          Restore previous value
                        </Button>
                      )}
                    </Stack>
                    <Typography variant="body2" sx={{ mt: 1 }}>
                      {revision.reason}
                    </Typography>
                    <Typography
                      component="pre"
                      variant="caption"
                      sx={{
                        p: 1.5,
                        bgcolor: 'background.default',
                        borderRadius: 1,
                        whiteSpace: 'pre-wrap',
                        overflowWrap: 'anywhere',
                      }}
                    >
                      {displayValue(revision.oldValue) || '(inherited)'} →{' '}
                      {displayValue(revision.newValue) || '(inherited)'}
                    </Typography>
                  </Box>
                ))
              ) : (
                <Typography color="text.secondary" sx={{ py: 6, textAlign: 'center' }}>
                  No changes recorded for this scope yet.
                </Typography>
              )}
            </Card>
          )}
        </>
      )}
      {draftKeys.length > 0 && (
        <Card
          role="status"
          sx={{
            position: { xs: 'static', sm: 'sticky' },
            bottom: 16,
            zIndex: 10,
            mt: 3,
            px: 3,
            py: 2,
            borderColor: 'primary.main',
            boxShadow: '0 6px 24px #16392818',
          }}
        >
          <Stack
            direction={{ xs: 'column', sm: 'row' }}
            alignItems={{ sm: 'center' }}
            justifyContent="space-between"
            spacing={2}
          >
            <Box>
              <Typography fontWeight={650}>
                {draftKeys.length} unsaved {draftKeys.length === 1 ? 'change' : 'changes'}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Review before applying to {scopeLabel.toLowerCase()}.
              </Typography>
            </Box>
            <Stack direction="row" spacing={1}>
              <Button disabled={saving} onClick={() => setConfirmation({ type: 'discard' })}>
                Discard changes
              </Button>
              <Button variant="contained" disabled={saving || hasError} onClick={review}>
                Review & save
              </Button>
            </Stack>
          </Stack>
        </Card>
      )}
      <Dialog
        open={!!confirmation}
        onClose={() => {
          if (!saving) setConfirmation(null);
        }}
        aria-labelledby="configuration-confirm-title"
      >
        <DialogTitle id="configuration-confirm-title">
          {confirmation?.type === 'scope' || confirmation?.type === 'discard'
            ? 'Discard unsaved changes?'
            : confirmation?.type === 'save'
              ? 'Review configuration changes'
              : 'Restore configuration value?'}
        </DialogTitle>
        <DialogContent>
          {confirmation?.type === 'scope' || confirmation?.type === 'discard' ? (
            <Typography>
              Your unsaved changes will be discarded
              {confirmation.type === 'scope' ? ' before switching scope' : ''}.
            </Typography>
          ) : (
            <>
              <Alert severity="info" sx={{ mb: 2 }}>
                These changes apply to {scopeLabel}.{' '}
                {locationId
                  ? 'Other locations keep their current settings.'
                  : 'Locations without overrides inherit these values.'}
              </Alert>
              {confirmation?.type === 'save' ? (
                draftKeys.map((key) => (
                  <Box key={key} sx={{ py: 1.5, borderBottom: 1, borderColor: 'divider' }}>
                    <Typography fontWeight={600} variant="body2">
                      {settingLabel(key)}
                    </Typography>
                    <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>
                      {catalog.data?.find((d) => d.key === key)?.sensitive
                        ? 'Hidden value will be updated'
                        : `${displayValue(resolved.get(key)?.value)} → ${drafts[key]?.raw ?? ''}`}
                    </Typography>
                  </Box>
                ))
              ) : (
                <Typography sx={{ mb: 2 }}>
                  Restore{' '}
                  {settingLabel(
                    confirmation?.type === 'revert'
                      ? confirmation.key
                      : confirmation?.type === 'rollback'
                        ? confirmation.revision.key
                        : '',
                  )}{' '}
                  {confirmation?.type === 'revert'
                    ? 'to its inherited value'
                    : 'to the value before this revision'}
                  .
                </Typography>
              )}
              <TextField
                autoFocus
                fullWidth
                label="Reason for this change"
                value={reason}
                onChange={(event) => setReason(event.target.value)}
                disabled={saving}
                helperText="At least 3 characters. Saved in the change history."
                sx={{ mt: 3 }}
              />
              {confirmation?.type === 'save' && draftKeys.length > 1 && (
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: 'block', mt: 1 }}
                >
                  Changes save individually. If one fails, remaining edits stay available for retry.
                </Typography>
              )}
            </>
          )}
        </DialogContent>
        <Divider />
        <DialogActions sx={{ p: 2 }}>
          <Button disabled={saving} onClick={() => setConfirmation(null)}>
            Cancel
          </Button>
          <Button
            variant="contained"
            disabled={
              saving ||
              (confirmation?.type !== 'scope' &&
                confirmation?.type !== 'discard' &&
                reason.trim().length < 3)
            }
            onClick={() => void confirm()}
          >
            {saving
              ? 'Saving…'
              : confirmation?.type === 'scope' || confirmation?.type === 'discard'
                ? 'Discard changes'
                : confirmation?.type === 'save'
                  ? 'Apply changes'
                  : 'Restore value'}
          </Button>
        </DialogActions>
      </Dialog>
    </PageShell>
  );
}
