import { MetricCard, OperationsShell, StatusBadge } from '@gaming-cafe/ui';
import { Add, ArrowForward } from '@mui/icons-material';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link as RouterLink } from 'react-router-dom';
import { getInventoryOverview, getReorderSuggestions } from '../../../services/inventory';
import { formatDisplayDateTime } from '../../../utils/date';

const currency = new Intl.NumberFormat('en-IN', {
  style: 'currency',
  currency: 'INR',
  maximumFractionDigits: 0,
});

export default function InventoryOverviewPage() {
  const overview = useQuery({ queryKey: ['inventory-overview'], queryFn: getInventoryOverview });
  const suggestions = useQuery({
    queryKey: ['reorder-suggestions'],
    queryFn: getReorderSuggestions,
  });

  if (overview.isError) {
    return (
      <Alert severity="error" sx={{ m: 4 }}>
        Inventory overview could not be loaded.
      </Alert>
    );
  }

  const data = overview.data;
  return (
    <OperationsShell
      header={{
        title: 'Inventory control center',
        description:
          'Stock health, procurement, approvals, and movement exceptions across every location.',
      }}
      actions={
        <Button
          component={RouterLink}
          to="/inventory/purchase-orders/new"
          variant="contained"
          startIcon={<Add />}
        >
          New purchase order
        </Button>
      }
    >
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,1fr)', lg: 'repeat(4,1fr)' },
          gap: 2,
        }}
      >
        <MetricCard
          label="Estimated stock value"
          value={data ? currency.format(data.estimatedStockValue) : '—'}
        />
        <MetricCard
          label="Low stock"
          value={data?.lowStockProducts ?? '—'}
          tone={data?.lowStockProducts ? 'warning' : 'success'}
        />
        <MetricCard
          label="Out of stock"
          value={data?.outOfStockProducts ?? '—'}
          tone={data?.outOfStockProducts ? 'error' : 'success'}
        />
        <MetricCard
          label="Open purchase orders"
          value={data?.openPurchaseOrders ?? '—'}
          tone="info"
        />
        <MetricCard
          label="Pending transfers"
          value={data?.pendingTransfers ?? '—'}
          tone={data?.pendingTransfers ? 'warning' : 'default'}
        />
        <MetricCard
          label="Waste approvals"
          value={data?.pendingWasteEvents ?? '—'}
          tone={data?.pendingWasteEvents ? 'warning' : 'default'}
        />
        <MetricCard
          label="Pieces on hand"
          value={data?.totalPieces?.toLocaleString('en-IN') ?? '—'}
        />
        <MetricCard
          label="Reorder suggestions"
          value={suggestions.data?.length ?? '—'}
          tone={suggestions.data?.length ? 'warning' : 'success'}
        />
      </Box>

      <Box
        sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', lg: '1fr 1fr' }, gap: 2, mt: 3 }}
      >
        <Paper variant="outlined" sx={{ p: 2.5 }}>
          <Stack direction="row" justifyContent="space-between" alignItems="center" mb={2}>
            <Typography variant="h6">Reorder suggestions</Typography>
            <Button component={RouterLink} to="/inventory/reorder" endIcon={<ArrowForward />}>
              Manage rules
            </Button>
          </Stack>
          <Stack spacing={1.5}>
            {(suggestions.data ?? []).slice(0, 6).map((item) => (
              <Stack key={item.ruleId} direction="row" justifyContent="space-between" gap={2}>
                <Box>
                  <Typography fontWeight={600}>{item.productName}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    {item.locationName} · {item.currentPieces} on hand
                  </Typography>
                </Box>
                <Stack direction="row" gap={1} alignItems="center">
                  <StatusBadge label={`Order ${item.suggestedPieces} pcs`} tone="warning" />
                  <Button
                    component={RouterLink}
                    size="small"
                    to={`/inventory/purchase-orders/new?locationId=${item.locationId}&productId=${item.productId}&pieces=${item.suggestedPieces}${item.preferredVendorId ? `&vendorId=${item.preferredVendorId}` : ''}`}
                  >
                    Create PO
                  </Button>
                </Stack>
              </Stack>
            ))}
            {!suggestions.isLoading && !suggestions.data?.length ? (
              <Typography color="text.secondary">
                All configured stock levels are healthy.
              </Typography>
            ) : null}
          </Stack>
        </Paper>

        <Paper variant="outlined" sx={{ p: 2.5 }}>
          <Stack direction="row" justifyContent="space-between" alignItems="center" mb={2}>
            <Typography variant="h6">Recent stock movements</Typography>
            <Button component={RouterLink} to="/inventory/movements" endIcon={<ArrowForward />}>
              View ledger
            </Button>
          </Stack>
          <Stack spacing={1.5}>
            {(data?.recentMovements ?? []).map((item) => (
              <Stack key={item.id} direction="row" justifyContent="space-between" gap={2}>
                <Box>
                  <Typography fontWeight={600}>{item.productName}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    {item.locationName} · {formatDisplayDateTime(item.createdAt)}
                  </Typography>
                </Box>
                <StatusBadge
                  label={`${item.delta > 0 ? '+' : ''}${item.delta} · ${item.movementType}`}
                  tone={item.delta >= 0 ? 'success' : 'warning'}
                />
              </Stack>
            ))}
            {!overview.isLoading && !data?.recentMovements.length ? (
              <Typography color="text.secondary">No stock movements yet.</Typography>
            ) : null}
          </Stack>
        </Paper>
      </Box>
    </OperationsShell>
  );
}
