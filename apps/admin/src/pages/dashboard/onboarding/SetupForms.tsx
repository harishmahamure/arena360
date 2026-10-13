import {
  DeviceStatus,
  deviceSubTypeOptions,
  deviceTypeOptions,
  Permission,
  PlanType,
} from '@gaming-cafe/contracts';
import { InfoOutlined } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Divider,
  FormControlLabel,
  IconButton,
  LinearProgress,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { type Ref, useImperativeHandle, useRef, useState } from 'react';
import { usePermissions } from '../../../hooks/usePermissions';
import { createMember, getAccess, saveMember, saveRole } from '../../../services/access';
import {
  createVenueLocation,
  getEffectiveSettings,
  getVenueLocations,
  putSettingOverride,
  type ResolvedSetting,
  updateVenueLocation,
  type VenueLocationDraft,
} from '../../../services/config';
import { addDevice } from '../../../services/devices/add';
import { getDevices } from '../../../services/devices/list';
import type { OnboardingSnapshot } from '../../../services/onboarding';
import { addPlan } from '../../../services/plans/add';

export interface SetupFormHandle {
  save: (exit?: boolean) => Promise<void>;
}
interface FormProps {
  organizationId: string;
  writable: boolean;
  disabled: boolean;
  formRef: Ref<SetupFormHandle>;
}
const fieldsSx = {
  display: 'grid',
  gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))' },
  gap: 3,
};

function Help({ text }: { text: string }) {
  return (
    <Tooltip title={text} enterTouchDelay={0}>
      <IconButton size="small" aria-label={text}>
        <InfoOutlined fontSize="small" />
      </IconButton>
    </Tooltip>
  );
}
function Section({ title, help }: { title: string; help: string }) {
  return (
    <Stack direction="row" alignItems="center" gap={1}>
      <Typography variant="h6" component="h2">
        {title}
      </Typography>
      <Help text={help} />
    </Stack>
  );
}
function LoadingError({ error, retry }: { error: Error | null; retry: () => void }) {
  return (
    error && (
      <Alert severity="error" action={<Button onClick={retry}>Retry</Button>}>
        {error.message}
      </Alert>
    )
  );
}

export function VenueForm({ organizationId, writable, disabled, formRef }: FormProps) {
  const { can } = usePermissions();
  const client = useQueryClient();
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId, 'onboarding'],
    queryFn: () => getVenueLocations(organizationId),
    enabled: can(Permission.LocationsRead),
    retry: false,
  });
  const [draft, setDraft] = useState<VenueLocationDraft | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  // Keep the form mounted while a save is retried; never replace a draft with fetched data.
  const saved = useRef<{ id: string; draft: VenueLocationDraft } | null>(null);
  const active = locations.data?.find(
    (location) => location.id === (selected ?? locations.data?.[0]?.id),
  );
  const value = draft ??
    active ?? { name: '', slug: '', timezone: 'Asia/Kolkata', currency: 'INR' };
  const write = writable && can(Permission.LocationsManage);
  useImperativeHandle(formRef, () => ({
    save: async (exit) => {
      if (!can(Permission.LocationsRead)) {
        if (exit) return;
        throw new Error('Location access is required to configure your venue.');
      }
      if (!locations.data) throw new Error('Wait for locations to load, then try again.');
      if (exit && !draft) return;
      if (!draft && active) return;
      if (!write)
        throw new Error('Location management permission is required to save this location.');
      if (!value.name.trim() || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value.slug))
        throw new Error(
          'Enter a location name and a location code using lowercase letters, numbers, and hyphens.',
        );
      try {
        new Intl.DateTimeFormat('en', { timeZone: value.timezone });
      } catch {
        throw new Error('Choose a valid timezone, such as Asia/Kolkata.');
      }
      if (!/^[A-Z]{3}$/.test(value.currency))
        throw new Error('Enter a three-letter currency code, such as INR.');
      const normalized = {
        name: value.name.trim(),
        slug: value.slug,
        timezone: value.timezone,
        currency: value.currency,
        isActive: true,
      };
      const id = saved.current?.id ?? active?.id;
      const result = id
        ? await updateVenueLocation(organizationId, id, normalized)
        : await createVenueLocation(organizationId, normalized);
      saved.current = { id: result.id, draft: normalized };
      setDraft(null);
      setSelected(result.id);
      client.setQueryData(
        ['venue-locations', organizationId, 'onboarding'],
        [result, ...(locations.data ?? []).filter((location) => location.id !== result.id)],
      );
      void client.invalidateQueries({ queryKey: ['venue-locations', organizationId] });
    },
  }));
  const change = (key: keyof VenueLocationDraft, next: string) =>
    setDraft({ ...value, [key]: next });
  if (!can(Permission.LocationsRead))
    return (
      <Alert severity="info">
        Ask an administrator with location access to configure the venue.
      </Alert>
    );
  return (
    <Stack gap={3}>
      <Section
        title="Your location"
        help="A location is the physical venue where your stations, staff, and shifts operate."
      />
      <Typography color="text.secondary">
        Enter the details of your first venue. You can add more locations after setup.
      </Typography>
      {locations.isPending && <LinearProgress aria-label="Loading locations" />}
      <LoadingError error={locations.error} retry={() => void locations.refetch()} />
      {!!locations.data?.length && (
        <TextField
          select
          label="Location"
          value={active?.id ?? ''}
          disabled={disabled}
          onChange={(event) => {
            setSelected(event.target.value);
            setDraft(null);
            saved.current = null;
          }}
        >
          {locations.data.map((location) => (
            <MenuItem key={location.id} value={location.id}>
              {location.name}
            </MenuItem>
          ))}
        </TextField>
      )}
      <Box
        component="fieldset"
        disabled={disabled || !write || !locations.data}
        sx={{ border: 0, p: 0, m: 0, minWidth: 0, ...fieldsSx }}
      >
        <TextField
          required
          label="Location name"
          value={value.name}
          helperText="The name your team sees, for example DualShock Arena."
          onChange={(event) => {
            const name = event.target.value;
            const previousSlug = value.name
              .toLowerCase()
              .replace(/[^a-z0-9]+/g, '-')
              .replace(/^-|-$/g, '');
            setDraft({
              ...value,
              name,
              slug:
                !value.slug || value.slug === previousSlug
                  ? name
                      .toLowerCase()
                      .replace(/[^a-z0-9]+/g, '-')
                      .replace(/^-|-$/g, '')
                  : value.slug,
            });
          }}
        />
        <TextField
          required
          label="Location code"
          value={value.slug}
          helperText="A unique short code, for example dualshock-arena. Use lowercase letters, numbers, and hyphens."
          onChange={(event) => change('slug', event.target.value)}
        />
        <TextField
          required
          label="Timezone"
          value={value.timezone}
          helperText="Used for shifts, receipts, and local pricing times. For India, use Asia/Kolkata."
          onChange={(event) => change('timezone', event.target.value)}
        />
        <TextField
          required
          label="Currency"
          value={value.currency}
          helperText="The currency for prices and payments, for example INR, USD, or EUR."
          onChange={(event) => change('currency', event.target.value.toUpperCase())}
        />
      </Box>
    </Stack>
  );
}

const settingFields = [
  {
    key: 'business.name',
    label: 'Business name',
    help: 'Shown on receipts and throughout your workspace.',
  },
  { key: 'business.address', label: 'Business address', help: 'The address printed on receipts.' },
  {
    key: 'business.phone',
    label: 'Contact phone',
    help: 'A phone number customers can use to reach your venue.',
  },
  {
    key: 'business.email',
    label: 'Contact email',
    help: 'An email address customers can use to reach your venue.',
  },
  {
    key: 'business.gst_number',
    label: 'GST registration number',
    help: 'Optional. Printed on receipts when GST details are enabled.',
  },
  {
    key: 'pricing.tax_rate',
    label: 'Tax rate (%)',
    help: 'The default tax percentage applied to pricing.',
    min: 0,
    max: 100,
  },
  {
    key: 'pricing.default_per_minute_rate',
    label: 'Default price per minute',
    help: 'The fallback price per minute when no specific pricing rule applies.',
    min: 0,
    max: 100000,
  },
  {
    key: 'pricing.night_window_start',
    label: 'Night pricing starts',
    help: 'Start of the night pricing window in the venue’s local time.',
    time: true,
  },
  {
    key: 'pricing.night_window_end',
    label: 'Night pricing ends',
    help: 'End of the night pricing window in the venue’s local time.',
    time: true,
  },
  {
    key: 'sessions.ending_warning_minutes',
    label: 'Session warning (minutes)',
    help: 'How many minutes before a session ends to warn your team.',
    min: 0,
    max: 120,
    integer: true,
  },
  {
    key: 'sessions.offline_grace_minutes',
    label: 'Offline grace period (minutes)',
    help: 'How long a session may continue while the connection is unavailable.',
    min: 0,
    max: 1440,
    integer: true,
  },
  {
    key: 'receipt.header_text',
    label: 'Receipt greeting',
    help: 'A message printed at the top of receipts.',
  },
  {
    key: 'receipt.footer_text',
    label: 'Receipt footer',
    help: 'A message printed at the bottom of receipts.',
  },
];

export function SettingsForm({ organizationId, writable, disabled, formRef }: FormProps) {
  const client = useQueryClient();
  const queryKey = ['effective-settings', organizationId, 'onboarding'];
  const settings = useQuery({
    queryKey,
    queryFn: () => getEffectiveSettings(organizationId),
    retry: false,
  });
  const [draft, setDraft] = useState<Record<string, string>>({});
  // Successfully saved fields keep their new revisions if a later field fails.
  const overrides = useRef<Record<string, ResolvedSetting>>({});
  useImperativeHandle(formRef, () => ({
    save: async () => {
      if (!settings.data) throw new Error('Wait for operating settings to load, then try again.');
      for (const field of settingFields) {
        if (!(field.key in draft)) continue;
        const current =
          overrides.current[field.key] ??
          settings.data.find((setting) => setting.key === field.key);
        if (!current) continue;
        const value = field.min !== undefined ? Number(draft[field.key]) : draft[field.key];
        if (value === current.value) continue;
        if (!writable)
          throw new Error('Settings write permission is required to save these changes.');
        if (
          field.min !== undefined &&
          (!draft[field.key]?.trim() ||
            !Number.isFinite(value) ||
            Number(value) < field.min ||
            Number(value) > (field.max ?? Infinity) ||
            (field.integer && !Number.isInteger(value)))
        )
          throw new Error(
            `Enter a valid ${field.label.toLowerCase()} between ${field.min} and ${field.max}.`,
          );
        if (field.time && !/^([01]\d|2[0-3]):[0-5]\d$/.test(String(value)))
          throw new Error(`Enter ${field.label.toLowerCase()} in HH:MM format.`);
        if (field.key === 'business.name' && !String(value).trim())
          throw new Error('Enter your business name.');
        const result = await putSettingOverride(organizationId, field.key, {
          value,
          reason: 'Initial venue setup',
          expectedRevision: current.sourceScope === 'organization' ? current.revision : 0,
        });
        const updated: ResolvedSetting = {
          ...current,
          value: result.value,
          revision: result.revision,
          sourceScope: 'organization',
        };
        overrides.current[field.key] = updated;
        client.setQueryData<ResolvedSetting[]>(queryKey, (previous) =>
          previous?.map((setting) => (setting.key === field.key ? updated : setting)),
        );
      }
      void client.invalidateQueries({ queryKey: ['branding'] });
      void client.invalidateQueries({ queryKey: ['effective-settings', organizationId] });
    },
  }));
  return (
    <Stack gap={3}>
      <Section
        title="How your venue operates"
        help="These defaults apply across your organization. Location-specific settings can be added later."
      />
      <Typography color="text.secondary">
        Review your business details, pricing, and session defaults. Keep any defaults that work for
        your venue.
      </Typography>
      {settings.isPending && <LinearProgress aria-label="Loading operating settings" />}
      <LoadingError error={settings.error} retry={() => void settings.refetch()} />
      <Box
        component="fieldset"
        disabled={disabled || !writable}
        sx={{ border: 0, p: 0, m: 0, minWidth: 0, ...fieldsSx }}
      >
        {settingFields
          .filter((field) => settings.data?.some((setting) => setting.key === field.key))
          .map((field) => (
            <TextField
              key={field.key}
              label={field.label}
              type={field.min !== undefined ? 'number' : field.time ? 'time' : 'text'}
              value={
                draft[field.key] ??
                String(settings.data?.find((setting) => setting.key === field.key)?.value ?? '')
              }
              helperText={field.help}
              slotProps={{
                inputLabel: { shrink: true },
                htmlInput: { min: field.min, max: field.max, step: field.integer ? 1 : 'any' },
              }}
              onChange={(event) => setDraft({ ...draft, [field.key]: event.target.value })}
            />
          ))}
      </Box>
      <Typography variant="body2" color="text.secondary">
        Save and continue confirms you have reviewed these operating settings.
      </Typography>
    </Stack>
  );
}

export function StationsForm({
  organizationId,
  writable,
  disabled,
  formRef,
  data,
}: FormProps & { data: OnboardingSnapshot }) {
  const { can } = usePermissions();
  const client = useQueryClient();
  const locations = useQuery({
    queryKey: ['venue-locations', organizationId, 'onboarding'],
    queryFn: () => getVenueLocations(organizationId),
    enabled: can(Permission.LocationsRead),
    retry: false,
  });
  const [location, setLocation] = useState('');
  const [station, setStation] = useState({
    name: '',
    deviceType: 'PC',
    deviceSubType: 'HIGH_END_PCS',
  });
  const [plan, setPlan] = useState({ name: '', price: '', timeCredits: '60', validityDays: '30' });
  const created = useRef({ station: '', plan: '' });
  const locationId =
    location ||
    data.readiness.locations.find((item) => item.stationCount > 0)?.id ||
    locations.data?.[0]?.id ||
    '';
  const devices = useQuery({
    queryKey: ['onboarding-devices', organizationId, locationId],
    queryFn: () => getDevices({ locationId, limit: 100 }),
    enabled: Boolean(locationId) && can(Permission.DevicesRead),
    retry: false,
  });
  const existing = devices.data?.data?.find(
    (device) =>
      device.locationId === locationId &&
      ['operational', 'available', 'in_use'].includes(device.status),
  );
  const stationCount =
    data.readiness.locations.find((item) => item.id === locationId)?.stationCount ?? 0;
  const hasStation = Boolean(created.current.station || stationCount);
  const hasPlan = Boolean(
    created.current.plan ||
      data.readiness.locations.find((item) => item.id === locationId)?.compatiblePlanCount,
  );
  useImperativeHandle(formRef, () => ({
    save: async (exit) => {
      if (exit && !station.name.trim() && !plan.name.trim()) return;
      if (!locationId)
        throw new Error('Create a location in the first step before adding stations.');
      if (!hasStation || station.name.trim()) {
        if (!created.current.station) {
          if (!writable || !can(Permission.DevicesWrite))
            throw new Error('Station write permission is required to add a station.');
          if (!station.name.trim()) throw new Error('Enter a station name, such as PC 01.');
          const result = await addDevice({
            ...station,
            name: station.name.trim(),
            locationId,
            status: DeviceStatus.OPERATIONAL,
          });
          created.current.station = result.id;
          void client.invalidateQueries({ queryKey: ['devices'] });
          void client.invalidateQueries({ queryKey: ['onboarding-devices', organizationId] });
        }
      }
      if (exit && !plan.name.trim()) return;
      if ((!hasPlan || plan.name.trim()) && !created.current.plan) {
        if (!writable || !can(Permission.PlansWrite))
          throw new Error('Plan write permission is required to add a plan.');
        if (
          !plan.name.trim() ||
          !plan.price.trim() ||
          !Number.isFinite(Number(plan.price)) ||
          Number(plan.price) < 0 ||
          !Number.isInteger(Number(plan.timeCredits)) ||
          Number(plan.timeCredits) <= 0 ||
          !Number.isInteger(Number(plan.validityDays)) ||
          Number(plan.validityDays) <= 0
        )
          throw new Error(
            'Enter a plan name, a valid price, and positive whole numbers for minutes and validity days.',
          );
        const target = created.current.station ? station : existing;
        if (!target)
          throw new Error(
            'Wait for stations to load or add a station so the plan can match its type.',
          );
        const result = await addPlan({
          name: plan.name.trim(),
          price: Number(plan.price),
          timeCredits: Number(plan.timeCredits),
          validityDays: Number(plan.validityDays),
          planType: PlanType.TIME_BASED,
          deviceType: target.deviceType,
          deviceSubType: target.deviceSubType,
          locationIds: [locationId],
          isActive: true,
        });
        created.current.plan = result.id;
        void client.invalidateQueries({ queryKey: ['plans'] });
      }
    },
  }));
  return (
    <Stack gap={3}>
      <Section
        title="Stations and plans"
        help="A station is a PC or console that players use. A plan gives players paid playing time on a matching station."
      />
      <Typography color="text.secondary">
        Set up one playable station and a time plan. You can add more after setup. Kiosk pairing is
        optional.
      </Typography>
      <LoadingError
        error={locations.error ?? devices.error}
        retry={() => {
          void locations.refetch();
          void devices.refetch();
        }}
      />
      <TextField
        select
        label="Station location"
        value={locations.data?.some((item) => item.id === locationId) ? locationId : ''}
        disabled={disabled || !!created.current.station}
        helperText="The station and plan will both belong to this location."
        onChange={(event) => {
          setLocation(event.target.value);
        }}
      >
        <MenuItem value="" disabled>
          Choose a location
        </MenuItem>
        {locations.data?.map((item) => (
          <MenuItem key={item.id} value={item.id}>
            {item.name}
          </MenuItem>
        ))}
      </TextField>
      {hasStation && (
        <Alert severity="success">
          This location already has a usable station. You can keep it or add another below.
        </Alert>
      )}
      <Box
        component="fieldset"
        disabled={
          disabled || !writable || !can(Permission.DevicesWrite) || !!created.current.station
        }
        sx={{ border: 0, p: 0, m: 0, minWidth: 0, ...fieldsSx }}
      >
        <TextField
          label="Station name"
          required={!hasStation}
          value={station.name}
          helperText="A name your team can recognize, for example PC 01 or PS5 01."
          onChange={(event) => setStation({ ...station, name: event.target.value })}
        />
        <TextField
          select
          label="Station type"
          value={station.deviceType}
          helperText="The kind of device players will use."
          onChange={(event) => setStation({ ...station, deviceType: event.target.value })}
        >
          {deviceTypeOptions.map((option) => (
            <MenuItem key={option.value} value={option.value}>
              {option.label}
            </MenuItem>
          ))}
        </TextField>
        <TextField
          select
          label="Station category"
          value={station.deviceSubType}
          helperText="Groups similar stations so plans can match their equipment."
          onChange={(event) => setStation({ ...station, deviceSubType: event.target.value })}
        >
          {deviceSubTypeOptions.map((option) => (
            <MenuItem key={option.value} value={option.value}>
              {option.label}
            </MenuItem>
          ))}
        </TextField>
      </Box>
      <Divider />
      <Section
        title="Your first time plan"
        help="Players buy this plan to get the specified minutes. It will match the station type and category at the selected location."
      />
      {hasPlan && (
        <Alert severity="success">
          A compatible plan is already available at this location. You can keep it or add another
          below.
        </Alert>
      )}
      <Box
        component="fieldset"
        disabled={disabled || !writable || !can(Permission.PlansWrite) || !!created.current.plan}
        sx={{ border: 0, p: 0, m: 0, minWidth: 0, ...fieldsSx }}
      >
        <TextField
          label="Plan name"
          required={!hasPlan}
          value={plan.name}
          helperText="For example, One hour gaming."
          onChange={(event) => setPlan({ ...plan, name: event.target.value })}
        />
        <TextField
          label={`Plan price (${locations.data?.find((item) => item.id === locationId)?.currency ?? 'venue currency'})`}
          type="number"
          required={!hasPlan}
          value={plan.price}
          slotProps={{ htmlInput: { min: 0, step: '0.01' } }}
          helperText="The amount a player pays for this plan."
          onChange={(event) => setPlan({ ...plan, price: event.target.value })}
        />
        <TextField
          label="Playing time (minutes)"
          type="number"
          value={plan.timeCredits}
          slotProps={{ htmlInput: { min: 1, step: 1 } }}
          helperText="60 minutes gives the player one hour of gaming."
          onChange={(event) => setPlan({ ...plan, timeCredits: event.target.value })}
        />
        <TextField
          label="Valid for (days)"
          type="number"
          value={plan.validityDays}
          slotProps={{ htmlInput: { min: 1, step: 1 } }}
          helperText="How long a player has to use their purchased minutes."
          onChange={(event) => setPlan({ ...plan, validityDays: event.target.value })}
        />
      </Box>
    </Stack>
  );
}

const operatorPermissions = [
  'locations:read',
  'devices:read',
  'plans:read',
  'shifts:read',
  'shifts:write',
  'cash-registers:read',
  'cash-registers:write',
  'players:read',
  'players:write',
  'transactions:read',
  'transactions:write',
  'player-plans:read',
  'player-plans:write',
  'sessions:read',
  'sessions:write',
];

export function OperatorForm({
  organizationId,
  writable,
  disabled,
  formRef,
  data,
}: FormProps & { data: OnboardingSnapshot }) {
  const { can } = usePermissions();
  const client = useQueryClient();
  const ready = Boolean(data.readiness.checks.find((check) => check.key === 'operator')?.ready);
  const access = useQuery({
    queryKey: ['onboarding-access', organizationId],
    queryFn: getAccess,
    enabled: !ready && can(Permission.AccessRead),
    retry: false,
  });
  const [memberId, setMemberId] = useState('');
  const [newMember, setNewMember] = useState(false);
  const [credentials, setCredentials] = useState({ username: '', password: '' });
  const [location, setLocation] = useState('');
  const roleId = useRef<string | null>(null);
  const saved = useRef(false);
  const locationId =
    location ||
    data.readiness.locations.find((item) => item.stationCount > 0 && item.compatiblePlanCount > 0)
      ?.id ||
    '';
  useImperativeHandle(formRef, () => ({
    save: async (exit) => {
      if (ready || saved.current || (exit && !memberId && !credentials.username)) return;
      if (!writable || !can(Permission.AccessManage))
        throw new Error('Team access management permission is required to assign an operator.');
      if (!access.data || !locationId)
        throw new Error('Finish stations and pricing, then choose an operator location.');
      const member = access.data.members.find((item) => item.id === memberId);
      if (!newMember && !member) throw new Error('Choose a team member to operate the venue.');
      const passwordBytes = new TextEncoder().encode(credentials.password).length;
      if (
        newMember &&
        (!/^[a-zA-Z0-9_.-]{3,80}$/.test(credentials.username.trim()) ||
          credentials.password.length < 12 ||
          passwordBytes > 72)
      )
        throw new Error(
          'Enter a username of 3–80 letters, numbers, dots, hyphens, or underscores and a password of at least 12 characters and at most 72 bytes.',
        );
      if (
        access.data.modules.some(
          (module) =>
            !module.enabled &&
            operatorPermissions.some((permission) => permission.startsWith(`${module.key}:`)),
        )
      )
        throw new Error(
          'Counter operation modules are disabled. Enable them in access management before assigning an operator.',
        );
      if (!roleId.current) {
        roleId.current =
          access.data.roles.find(
            (role) =>
              !role.isTemplate &&
              operatorPermissions.every((permission) => role.permissions.includes(permission)) &&
              role.permissions.every((permission) => operatorPermissions.includes(permission)),
          )?.id ?? null;
        if (!roleId.current) {
          const names = new Set(
            access.data.roles
              .filter((role) => !role.isTemplate)
              .map((role) => role.name.toLowerCase()),
          );
          let name = 'Counter operator';
          for (let suffix = 2; names.has(name.toLowerCase()); suffix++)
            name = `Counter operator ${suffix}`;
          roleId.current = (
            await saveRole({
              name,
              description: 'Operate shifts, sales, and gaming sessions at an assigned location.',
              permissions: operatorPermissions,
              isTemplate: false,
            }).catch(async (error) => {
              await access.refetch();
              throw error;
            })
          ).id;
        }
      }
      const id = roleId.current;
      if (newMember) {
        await createMember({
          username: credentials.username.trim(),
          password: credentials.password,
          roleIds: [id],
          locationIds: [locationId],
          locationRoles: [{ locationId, roleIds: [id] }],
        });
        setCredentials({ username: '', password: '' });
      } else if (member) {
        const scopes =
          member.locationRoles ??
          member.locationIds.map((locationId) => ({ locationId, roleIds: member.roleIds }));
        const selected = scopes.find((scope) => scope.locationId === locationId);
        await saveMember({
          ...member,
          active: true,
          locationIds: [...new Set([...member.locationIds, locationId])],
          locationRoles: [
            ...scopes.filter((scope) => scope.locationId !== locationId),
            {
              locationId,
              roleIds: [
                ...new Set([
                  ...(selected?.roleIds ?? []),
                  ...member.roleIds.filter(
                    (role) => !scopes.some((scope) => scope.roleIds.includes(role)),
                  ),
                  id,
                ]),
              ],
            },
          ],
        });
      }
      saved.current = true;
      void client.invalidateQueries({ queryKey: ['accessManagement'] });
      void client.invalidateQueries({ queryKey: ['onboarding-access', organizationId] });
      void client.invalidateQueries({ queryKey: ['myAccess'] });
    },
  }));
  if (ready)
    return <Alert severity="success">An operator already has access to run your venue.</Alert>;
  if (!can(Permission.AccessRead))
    return (
      <Alert severity="info">
        Ask an administrator with team access to assign a counter operator.
      </Alert>
    );
  return (
    <Stack gap={3}>
      <Section
        title="Who will operate this venue?"
        help="An operator needs permission to open shifts, manage cash, sell plans, and run player sessions at this location."
      />
      <Typography color="text.secondary">
        Assign counter operator access to an existing team member or create a new operator account.
      </Typography>
      {access.isPending && <LinearProgress aria-label="Loading team access" />}
      <LoadingError error={access.error} retry={() => void access.refetch()} />
      <Box
        component="fieldset"
        disabled={disabled || !writable || !can(Permission.AccessManage) || saved.current}
        sx={{ border: 0, p: 0, m: 0, minWidth: 0 }}
      >
        <Stack gap={3}>
          <TextField
            select
            label="Operator location"
            value={locationId}
            helperText="Choose the location with a station and a compatible plan."
            onChange={(event) => setLocation(event.target.value)}
          >
            <MenuItem value="" disabled>
              Choose a location
            </MenuItem>
            {data.readiness.locations
              .filter((item) => item.stationCount > 0 && item.compatiblePlanCount > 0)
              .map((item) => (
                <MenuItem key={item.id} value={item.id}>
                  {item.name}
                </MenuItem>
              ))}
          </TextField>
          <FormControlLabel
            label="Create a new operator account"
            control={
              <Checkbox checked={newMember} onChange={(_, checked) => setNewMember(checked)} />
            }
          />
          {newMember ? (
            <Box sx={fieldsSx}>
              <TextField
                label="Operator username"
                autoComplete="off"
                value={credentials.username}
                helperText="3–80 letters, numbers, dots, hyphens, or underscores."
                onChange={(event) =>
                  setCredentials({ ...credentials, username: event.target.value })
                }
              />
              <TextField
                label="Initial password"
                type="password"
                autoComplete="new-password"
                value={credentials.password}
                helperText="At least 12 characters. Share it securely with the operator."
                onChange={(event) =>
                  setCredentials({ ...credentials, password: event.target.value })
                }
              />
            </Box>
          ) : (
            <TextField
              select
              label="Counter operator"
              value={memberId}
              helperText="This person will receive counter and session access at the chosen location."
              onChange={(event) => setMemberId(event.target.value)}
            >
              <MenuItem value="" disabled>
                Choose an operator
              </MenuItem>
              {access.data?.members.map((member) => (
                <MenuItem key={member.id} value={member.id}>
                  {member.name || member.username}
                </MenuItem>
              ))}
            </TextField>
          )}
        </Stack>
      </Box>
    </Stack>
  );
}
