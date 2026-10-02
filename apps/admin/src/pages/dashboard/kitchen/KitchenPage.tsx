import { PageHeader, PageShell } from '@gaming-cafe/ui';
import { Refresh, Restaurant } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
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
import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { GuidedForm, GuidedStep } from '../../../components/GuidedForm';
import { usePermissions } from '../../../hooks/usePermissions';
import {
  advanceKitchenTicket,
  getKitchenMenu,
  getKitchenTickets,
  type KitchenMenuItem,
  type KitchenStatus,
  type KitchenTicket,
  saveKitchenMenu,
} from '../../../services/operations';

const labels: Record<KitchenStatus, string> = {
  queued: 'Queued',
  preparing: 'Preparing',
  ready: 'Ready to serve',
  served: 'Served',
  cancelled: 'Cancelled',
};
const next: Partial<Record<KitchenStatus, KitchenStatus>> = {
  queued: 'preparing',
  preparing: 'ready',
  ready: 'served',
};
const actions: Partial<Record<KitchenStatus, string>> = {
  queued: 'Start preparing',
  preparing: 'Mark ready',
  ready: 'Mark served',
};

export default function KitchenPage() {
  const { isAdmin } = usePermissions();
  const client = useQueryClient();
  const [tab, setTab] = useState('queue');
  const [search, setSearch] = useState('');
  const [station, setStation] = useState('all');
  const [now, setNow] = useState(Date.now());
  const [selected, setSelected] = useState<KitchenTicket | null>(null);
  const [cancel, setCancel] = useState<KitchenTicket | null>(null);
  const [reason, setReason] = useState('');
  const [editing, setEditing] = useState<KitchenMenuItem | null>(null);
  const tickets = useQuery({
    queryKey: ['kitchen', tab === 'history'],
    queryFn: () => getKitchenTickets(tab === 'history'),
    enabled: tab !== 'menu',
    refetchInterval: 15000,
  });
  const menu = useQuery({
    queryKey: ['kitchenMenu'],
    queryFn: getKitchenMenu,
    enabled: isAdmin && tab === 'menu',
  });
  const advance = useMutation({
    mutationFn: ({
      ticket,
      status,
      reason,
    }: {
      ticket: KitchenTicket;
      status: KitchenStatus;
      reason?: string;
    }) => advanceKitchenTicket(ticket, status, reason),
    onSuccess: () => {
      setCancel(null);
      setReason('');
      void client.invalidateQueries({ queryKey: ['kitchen'] });
    },
    onError: () => {
      void client.invalidateQueries({ queryKey: ['kitchen'] });
    },
  });
  const save = useMutation({
    mutationFn: saveKitchenMenu,
    onSuccess: () => {
      setEditing(null);
      void client.invalidateQueries({ queryKey: ['kitchenMenu'] });
    },
  });
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 15000);
    return () => clearInterval(timer);
  }, []);
  const all = tickets.data ?? [];
  const stations = [
    ...new Set(all.flatMap((ticket) => ticket.items.map((item) => item.station))),
  ].sort();
  const filtered = all.filter(
    (ticket) =>
      (station === 'all' || ticket.items.some((item) => item.station === station)) &&
      `${ticket.id} ${ticket.customer} ${ticket.items.map((item) => item.name).join(' ')}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const details = all.find((ticket) => ticket.id === selected?.id) ?? selected;
  const error = tab === 'menu' ? menu.error : tickets.error;
  const loading = tab === 'menu' ? menu.isFetching : tickets.isFetching;
  function ticketCard(ticket: KitchenTicket) {
    const overdue =
      now > Date.parse(ticket.dueAt) && ['queued', 'preparing'].includes(ticket.status);
    const validPayment = ['completed', 'credit'].includes(ticket.paymentStatus);
    return (
      <Card
        key={ticket.id}
        variant="outlined"
        sx={{ borderColor: overdue ? 'warning.main' : 'divider' }}
      >
        <CardContent>
          <Stack direction="row" justifyContent="space-between" gap={1}>
            <Typography fontWeight={700}>#{ticket.id.slice(0, 8)}</Typography>
            <Chip
              size="small"
              label={`${Math.max(0, Math.floor((now - Date.parse(ticket.createdAt)) / 60000))} min`}
              color={overdue ? 'warning' : 'default'}
            />
          </Stack>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            {ticket.customer} ·{' '}
            {new Date(ticket.createdAt).toLocaleTimeString([], {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </Typography>
          {ticket.items.map((item, index) => (
            <Box key={`${item.productId}-${index}`} sx={{ mb: 1, overflowWrap: 'anywhere' }}>
              <Typography fontWeight={600}>
                {item.quantity} × {item.name}
              </Typography>
              {item.options?.length ? (
                <Typography variant="body2">{item.options.join(', ')}</Typography>
              ) : null}
              <Typography variant="caption" color="text.secondary">
                {item.station}
              </Typography>
            </Box>
          ))}
          {ticket.notes && (
            <Alert severity="info" icon={false} sx={{ my: 1, overflowWrap: 'anywhere' }}>
              {ticket.notes}
            </Alert>
          )}
          {overdue && (
            <Typography variant="caption" color="warning.main">
              Preparation target exceeded
            </Typography>
          )}
          {!validPayment && (
            <Alert severity="warning">
              Payment {ticket.paymentStatus}. Review the sale before continuing.
            </Alert>
          )}
          {next[ticket.status] && (
            <Button
              fullWidth
              variant="contained"
              sx={{ mt: 2 }}
              disabled={advance.isPending || tickets.isError || !validPayment}
              onClick={() => advance.mutate({ ticket, status: next[ticket.status]! })}
            >
              {actions[ticket.status]}
            </Button>
          )}
          <Stack direction="row" justifyContent="space-between" sx={{ mt: 1 }}>
            <Button size="small" onClick={() => setSelected(ticket)}>
              Details & history
            </Button>
            {next[ticket.status] && (
              <Button
                size="small"
                color="error"
                disabled={advance.isPending}
                onClick={() => {
                  advance.reset();
                  setCancel(ticket);
                }}
              >
                Cancel
              </Button>
            )}
          </Stack>
        </CardContent>
      </Card>
    );
  }
  return (
    <PageShell>
      <PageHeader
        title="Kitchen"
        description="One queue from sale to service. Actions update the whole ticket, including all prep stations."
      />
      <Stack direction="row" flexWrap="wrap" gap={1} sx={{ mb: 2 }}>
        <Chip
          icon={<Restaurant />}
          label={`${all.filter((t) => t.status === 'queued').length} waiting`}
        />
        <Chip label={`${all.filter((t) => t.status === 'preparing').length} preparing`} />
        <Chip color="success" label={`${all.filter((t) => t.status === 'ready').length} ready`} />
        <Box sx={{ flex: 1 }} />
        <Button
          startIcon={<Refresh />}
          disabled={loading}
          onClick={() => void (tab === 'menu' ? menu.refetch() : tickets.refetch())}
        >
          Refresh
        </Button>
        <Button component={Link} to="/kiosk-orders">
          Kiosk orders
        </Button>
      </Stack>
      <Tabs
        value={tab}
        onChange={(_, value) => {
          setTab(value);
          setStation('all');
          setSearch('');
          advance.reset();
        }}
        variant="scrollable"
      >
        <Tab value="queue" label="Live queue" />
        <Tab value="history" label="Recent history" />
        {isAdmin && <Tab value="menu" label="Menu setup" />}
      </Tabs>
      {loading && <LinearProgress aria-label="Loading kitchen" />}
      {error && (
        <Alert
          severity="error"
          sx={{ mt: 2 }}
          action={
            <Button onClick={() => void (tab === 'menu' ? menu.refetch() : tickets.refetch())}>
              Retry
            </Button>
          }
        >
          {error instanceof Error ? error.message : 'Could not load kitchen.'} Actions are paused
          until the queue refreshes.
        </Alert>
      )}
      {advance.error && !cancel && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {advance.error.message}
        </Alert>
      )}
      <Stack direction={{ xs: 'column', sm: 'row' }} gap={2} sx={{ my: 2 }}>
        <TextField
          label={tab === 'menu' ? 'Find menu item' : 'Find ticket, customer or item'}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          fullWidth
        />
        {tab !== 'menu' && (
          <TextField
            select
            label="Prep station"
            value={station}
            onChange={(e) => setStation(e.target.value)}
            sx={{ minWidth: 190 }}
          >
            <MenuItem value="all">All stations</MenuItem>
            {stations.map((s) => (
              <MenuItem key={s} value={s}>
                {s}
              </MenuItem>
            ))}
          </TextField>
        )}
      </Stack>
      {tab === 'menu' ? (
        <>
          <Alert severity="info" sx={{ mb: 2 }}>
            Enable products that need preparation. New completed or credit sales will create tickets
            automatically. Existing sales and tickets are unchanged.
          </Alert>
          <Stack gap={1}>
            {menu.data
              ?.filter((item) => item.name.toLowerCase().includes(search.toLowerCase()))
              .map((item) => (
                <Card key={item.productId} variant="outlined">
                  <CardContent>
                    <Stack direction="row" gap={2} alignItems="center">
                      <Box sx={{ flex: 1, minWidth: 0 }}>
                        <Typography fontWeight={600}>{item.name}</Typography>
                        <Typography variant="body2" color="text.secondary">
                          {item.enabled
                            ? `${item.station} · ${item.prepMinutes} minute target`
                            : 'No kitchen preparation'}
                        </Typography>
                      </Box>
                      <Button
                        onClick={() => {
                          save.reset();
                          setEditing({ ...item });
                        }}
                      >
                        Configure
                      </Button>
                    </Stack>
                  </CardContent>
                </Card>
              ))}
          </Stack>
          {menu.data?.length === 0 && (
            <Alert severity="info">
              Add products to the catalog before configuring kitchen preparation.
            </Alert>
          )}
        </>
      ) : (
        <>
          {tab === 'history' && (
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              Served and cancelled tickets from the last 7 days, up to 500 tickets.
            </Typography>
          )}
          {all.length >= 500 && (
            <Alert severity="warning">
              Showing up to 500 tickets. The live queue prioritizes the oldest orders.
            </Alert>
          )}
          {!tickets.isPending && !error && filtered.length === 0 && (
            <Alert severity="info">
              {search || station !== 'all'
                ? 'No tickets match these filters.'
                : 'No tickets here yet. Kitchen-enabled items appear after a completed or credit sale.'}
            </Alert>
          )}
          {tab === 'queue' ? (
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: { xs: '1fr', md: 'repeat(3,minmax(0,1fr))' },
                gap: 2,
              }}
            >
              {(['queued', 'preparing', 'ready'] as const).map((status) => (
                <Box key={status} sx={{ minWidth: 0 }}>
                  <Typography component="h2" variant="h6" sx={{ mb: 2 }}>
                    {labels[status]} · {filtered.filter((t) => t.status === status).length}
                  </Typography>
                  <Stack gap={2}>
                    {filtered.filter((t) => t.status === status).map(ticketCard)}
                  </Stack>
                </Box>
              ))}
            </Box>
          ) : (
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: { xs: '1fr', md: 'repeat(3,minmax(0,1fr))' },
                gap: 2,
              }}
            >
              {filtered.map((ticket) => (
                <Box key={ticket.id}>
                  <Chip label={labels[ticket.status]} sx={{ mb: 1 }} />
                  {ticketCard(ticket)}
                </Box>
              ))}
            </Box>
          )}
        </>
      )}
      <Dialog open={!!selected} onClose={() => setSelected(null)} fullWidth>
        <DialogTitle>Ticket details</DialogTitle>
        <DialogContent>
          {details && (
            <Stack gap={2}>
              <Typography>
                {details.customer} · #{details.id.slice(0, 8)}
              </Typography>
              <Button component={Link} to={`/product-transactions/${details.transactionId}`}>
                Open sale
              </Button>
              <Divider />
              {details.events.map((event, i) => (
                <Box key={i}>
                  <Typography fontWeight={600}>
                    {labels[event.status]} · {event.actor ?? 'System'}
                  </Typography>
                  <Typography variant="body2">{new Date(event.at).toLocaleString()}</Typography>
                  {event.reason && (
                    <Typography sx={{ overflowWrap: 'anywhere' }}>{event.reason}</Typography>
                  )}
                </Box>
              ))}
            </Stack>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setSelected(null)}>Close</Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={!!cancel}
        onClose={() => {
          if (!advance.isPending) setCancel(null);
        }}
        fullWidth
      >
        <DialogTitle>Cancel kitchen ticket</DialogTitle>
        <DialogContent>
          <Alert severity="warning" sx={{ mb: 2 }}>
            This stops preparation only. It does not refund the sale or return stock. Handle those
            separately.
          </Alert>
          {advance.error && <Alert severity="error">{advance.error.message}</Alert>}
          <TextField
            autoFocus
            fullWidth
            multiline
            minRows={2}
            label="Reason"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            inputProps={{ maxLength: 500 }}
            disabled={advance.isPending}
          />
        </DialogContent>
        <DialogActions>
          <Button disabled={advance.isPending} onClick={() => setCancel(null)}>
            Keep ticket
          </Button>
          <Button
            color="error"
            variant="contained"
            disabled={!reason.trim() || advance.isPending}
            onClick={() =>
              cancel &&
              advance.mutate({ ticket: cancel, status: 'cancelled', reason: reason.trim() })
            }
          >
            Cancel ticket
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog
        open={!!editing}
        onClose={() => {
          if (!save.isPending) setEditing(null);
        }}
        fullWidth
      >
        <DialogTitle>Configure {editing?.name}</DialogTitle>
        <DialogContent>
          {editing && (
            <GuidedForm
              busy={save.isPending}
              onCancel={() => setEditing(null)}
              review={
                <Typography>
                  {editing.enabled
                    ? `${editing.name} → ${editing.station}, ${editing.prepMinutes} minute preparation target`
                    : `${editing.name} will not create kitchen tickets.`}
                </Typography>
              }
              actions={
                <Button
                  variant="contained"
                  disabled={save.isPending}
                  onClick={() => save.mutate(editing)}
                >
                  Save kitchen setup
                </Button>
              }
            >
              <GuidedStep
                title="Preparation"
                description="Choose whether this item needs kitchen preparation."
              >
                <FormControlLabel
                  control={
                    <Switch
                      checked={editing.enabled}
                      onChange={(e) =>
                        setEditing({
                          ...editing,
                          enabled: e.target.checked,
                          station: editing.station.trim() || 'Kitchen',
                          prepMinutes:
                            Number.isInteger(editing.prepMinutes) &&
                            editing.prepMinutes >= 1 &&
                            editing.prepMinutes <= 240
                              ? editing.prepMinutes
                              : 15,
                        })
                      }
                    />
                  }
                  label="Send to kitchen after sale"
                />
              </GuidedStep>
              <GuidedStep
                title="Station & timing"
                validate={() =>
                  editing.enabled &&
                  (!editing.station.trim() ||
                    !Number.isInteger(editing.prepMinutes) ||
                    editing.prepMinutes < 1 ||
                    editing.prepMinutes > 240)
                    ? 'Choose a station and a whole preparation time from 1 to 240 minutes.'
                    : undefined
                }
              >
                <Stack gap={2}>
                  <TextField
                    label="Prep station"
                    required
                    disabled={!editing.enabled}
                    value={editing.station}
                    inputProps={{ maxLength: 60 }}
                    onChange={(e) => setEditing({ ...editing, station: e.target.value })}
                  />
                  <TextField
                    label="Preparation target (minutes)"
                    type="number"
                    required
                    disabled={!editing.enabled}
                    inputProps={{ min: 1, max: 240, step: 1 }}
                    value={editing.prepMinutes}
                    onChange={(e) =>
                      setEditing({ ...editing, prepMinutes: Number(e.target.value) })
                    }
                  />
                </Stack>
              </GuidedStep>
            </GuidedForm>
          )}
          {save.error && (
            <Alert
              severity="error"
              sx={{ mt: 2 }}
              action={
                <Button
                  onClick={() => {
                    setEditing(null);
                    void menu.refetch();
                  }}
                >
                  Close & reload
                </Button>
              }
            >
              {save.error.message}
            </Alert>
          )}
        </DialogContent>
      </Dialog>
    </PageShell>
  );
}
