import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { Link as RouterLink, useNavigate } from 'react-router-dom';
import { formatRelativeTime } from '../../../components/notifications/notificationUtils';
import {
  getKioskOrders,
  type KioskOrder,
  updateKioskOrderStatus,
} from '../../../services/kiosk-orders';

export const PENDING_KIOSK_ORDERS_QUERY_KEY = ['kiosk-orders', 'open'] as const;

function statusColor(status: string): 'default' | 'warning' | 'success' | 'error' {
  switch (status) {
    case 'pending':
      return 'warning';
    case 'preparing':
      return 'default';
    case 'fulfilled':
      return 'success';
    case 'cancelled':
      return 'error';
    default:
      return 'default';
  }
}

function OrderActions({ order }: { order: KioskOrder }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const mutation = useMutation({
    mutationFn: (status: string) => updateKioskOrderStatus(order.id, status),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['kiosk-orders'] });
    },
  });

  const isOpen = order.status === 'pending' || order.status === 'preparing';

  if (!isOpen) {
    return order.transactionId ? (
      <Button size="small" onClick={() => navigate(`/product-transactions/${order.transactionId}`)}>
        View sale
      </Button>
    ) : null;
  }

  return (
    <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap justifyContent="flex-end">
      {order.status === 'pending' ? (
        <Button
          size="small"
          variant="outlined"
          disabled={mutation.isPending}
          onClick={() => mutation.mutate('preparing')}
        >
          Preparing
        </Button>
      ) : null}
      <Button
        size="small"
        variant="contained"
        onClick={() => navigate(`/product-transactions/new?orderId=${order.id}`)}
      >
        Convert to sale
      </Button>
      <Button
        size="small"
        color="error"
        disabled={mutation.isPending}
        onClick={() => mutation.mutate('cancelled')}
      >
        Cancel
      </Button>
    </Stack>
  );
}

interface PendingKioskOrdersPanelProps {
  /** Full page includes title and description; embedded is for dashboard footer. */
  variant?: 'page' | 'embedded';
}

export function PendingKioskOrdersPanel({ variant = 'page' }: PendingKioskOrdersPanelProps) {
  const { data, isLoading, error } = useQuery({
    queryKey: PENDING_KIOSK_ORDERS_QUERY_KEY,
    queryFn: () => getKioskOrders({ limit: 50 }),
    refetchInterval: 30_000,
  });

  const orders = useMemo(() => {
    const all = data?.data ?? [];
    return all.filter((o) => o.status === 'pending' || o.status === 'preparing');
  }, [data]);

  const isEmbedded = variant === 'embedded';

  const content = (() => {
    if (error) {
      return <Alert severity="error">Could not load kiosk orders.</Alert>;
    }
    if (isLoading) {
      return <CircularProgress size={32} />;
    }
    if (orders.length === 0) {
      return (
        <Paper variant="outlined" sx={{ p: isEmbedded ? 2 : 3 }}>
          <Typography color="text.secondary">No open orders right now.</Typography>
        </Paper>
      );
    }
    return (
      <TableContainer component={Paper} variant="outlined">
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>When</TableCell>
              <TableCell>PC</TableCell>
              <TableCell>Player</TableCell>
              <TableCell>Items</TableCell>
              <TableCell>Status</TableCell>
              <TableCell align="right">Actions</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {orders.map((order) => (
              <TableRow key={order.id} hover>
                <TableCell>{formatRelativeTime(order.createdAt)}</TableCell>
                <TableCell>{order.deviceName ?? order.deviceId.slice(0, 8)}</TableCell>
                <TableCell>{order.playerUsername ?? order.playerId.slice(0, 8)}</TableCell>
                <TableCell>
                  {order.lineItems.map((i) => `${i.quantity}× ${i.productName}`).join(', ')}
                  {order.playerNote ? (
                    <Typography variant="caption" display="block" color="text.secondary">
                      Note: {order.playerNote}
                    </Typography>
                  ) : null}
                </TableCell>
                <TableCell>
                  <Chip size="small" label={order.status} color={statusColor(order.status)} />
                </TableCell>
                <TableCell align="right">
                  <OrderActions order={order} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableContainer>
    );
  })();

  if (isEmbedded) {
    return (
      <Box sx={{ mt: 4 }}>
        <Stack
          direction="row"
          alignItems="center"
          justifyContent="space-between"
          flexWrap="wrap"
          gap={1}
          sx={{ mb: 1.5 }}
        >
          <Typography variant="subtitle1" fontWeight={600}>
            Customer orders pending
            {orders.length > 0 ? (
              <Chip
                size="small"
                label={orders.length}
                color="warning"
                sx={{ ml: 1, verticalAlign: 'middle' }}
              />
            ) : null}
          </Typography>
          <Button component={RouterLink} to="/kiosk-orders" size="small" variant="text">
            View all orders
          </Button>
        </Stack>
        {content}
      </Box>
    );
  }

  return (
    <Box sx={{ p: 3 }}>
      <Typography variant="h5" gutterBottom>
        Kiosk orders
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        Food and drink orders placed from gaming stations. Fulfill manually, then convert to a POS
        sale.
      </Typography>
      {content}
    </Box>
  );
}
