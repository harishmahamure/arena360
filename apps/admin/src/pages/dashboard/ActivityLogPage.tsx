import {
  ACTIVITY_KIND_VALUES,
  type ActivityKindValue,
  activityKindLabels,
} from '@gaming-cafe/contracts';
import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { formatRemainingLabel } from '@gaming-cafe/utils';
import { Visibility } from '@mui/icons-material';
import {
  Alert,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import {
  formatRelativeTime,
  getNotificationLink,
  kindLabel,
} from '../../components/notifications/notificationUtils';
import { type ActivityLogEntry, getActivityLog } from '../../services/notifications';
import { buildListUrl } from '../../utils/buildListUrl';
import { formatDisplayDateTime } from '../../utils/date';

export default function ActivityLogPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const page = Number(searchParams.get('page') || '1');
  const kind = searchParams.get('kind') || '';
  const from = searchParams.get('from') || '';
  const to = searchParams.get('to') || '';

  const { data, isLoading, error } = useQuery({
    queryKey: ['activity-log', page, kind, from, to],
    queryFn: () =>
      getActivityLog({
        page,
        kind: kind || undefined,
        from: from || undefined,
        to: to || undefined,
      }),
  });

  const updateFilter = (key: string, value: string) => {
    const next = new URLSearchParams(searchParams);
    if (value) {
      next.set(key, value);
    } else {
      next.delete(key);
    }
    next.delete('page');
    setSearchParams(next);
  };

  const columns: Column<ActivityLogEntry>[] = [
    {
      id: 'createdAt',
      label: 'When',
      minWidth: 180,
      format: (value) => (
        <>
          <Typography variant="body2">{formatDisplayDateTime(value as string)}</Typography>
          <Typography variant="caption" color="text.secondary">
            {formatRelativeTime(value as string)}
          </Typography>
        </>
      ),
    },
    {
      id: 'kind',
      label: 'Type',
      minWidth: 140,
      format: (value) => kindLabel(value as string),
    },
    {
      id: 'title',
      label: 'Title',
      minWidth: 200,
    },
    {
      id: 'summary',
      label: 'Summary',
      minWidth: 220,
      hideOnMobile: true,
      format: (value, row) => {
        const summary = (value as string | undefined) || '';
        if (row.kind !== 'session_started') {
          return summary || '—';
        }
        const payload = row.payload as Record<string, unknown> | undefined;
        const loginMinutes =
          typeof payload?.walletMinutesAtStart === 'number' ? payload.walletMinutesAtStart : null;
        if (loginMinutes == null) {
          return summary || '—';
        }
        const loginLabel = formatRemainingLabel(loginMinutes);
        if (summary.includes('at login')) {
          return summary;
        }
        return summary ? `${summary} · ${loginLabel} at login` : `${loginLabel} at login`;
      },
    },
  ];

  const actions: Action<ActivityLogEntry>[] = [
    {
      label: 'View',
      icon: <Visibility />,
      onClick: (row) => {
        const link = getNotificationLink(row.entityType, row.entityId, row.payload);
        if (link) navigate(link);
      },
    },
  ];

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load activity log.
        </Alert>
      )}

      <Stack
        direction={{ xs: 'column', md: 'row' }}
        spacing={2}
        sx={{ px: { xs: 2, md: 4 }, pt: { xs: 2, md: 3 }, pb: 1 }}
      >
        <FormControl size="small" sx={{ minWidth: 200 }}>
          <InputLabel id="activity-kind-filter">Event type</InputLabel>
          <Select
            labelId="activity-kind-filter"
            label="Event type"
            value={kind}
            onChange={(e) => updateFilter('kind', e.target.value)}
          >
            <MenuItem value="">All types</MenuItem>
            {ACTIVITY_KIND_VALUES.map((value) => (
              <MenuItem key={value} value={value}>
                {activityKindLabels[value as ActivityKindValue]}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <TextField
          size="small"
          type="date"
          label="From"
          InputLabelProps={{ shrink: true }}
          value={from ? from.slice(0, 10) : ''}
          onChange={(e) =>
            updateFilter('from', e.target.value ? `${e.target.value}T00:00:00Z` : '')
          }
        />
        <TextField
          size="small"
          type="date"
          label="To"
          InputLabelProps={{ shrink: true }}
          value={to ? to.slice(0, 10) : ''}
          onChange={(e) => updateFilter('to', e.target.value ? `${e.target.value}T23:59:59Z` : '')}
        />
      </Stack>

      <ListPage<ActivityLogEntry>
        title="Activity log"
        description="Important operational events across the cafe."
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        showSearch={false}
        emptyMessage="No activity recorded yet."
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(
              buildListUrl('/activity-log', value, {
                kind,
                from,
                to,
              }),
            ),
        }}
      />
    </>
  );
}
