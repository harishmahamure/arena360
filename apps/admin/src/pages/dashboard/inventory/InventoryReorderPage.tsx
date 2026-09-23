import { OperationsShell, StatusBadge } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Autocomplete,
  Button,
  MenuItem,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { usePermissions } from '../../../hooks/usePermissions';
import {
  getInventoryLocations,
  getReorderRules,
  getReorderSuggestions,
  saveReorderRule,
} from '../../../services/inventory';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { getVendors } from '../../../services/vendors';

export default function InventoryReorderPage() {
  const { isAdmin } = usePermissions();
  const client = useQueryClient();
  const [locationId, setLocationId] = useState('');
  const [product, setProduct] = useState<ProductResponse | null>(null);
  const [minimum, setMinimum] = useState(0);
  const [target, setTarget] = useState(0);
  const [vendorId, setVendorId] = useState('');
  const [leadTime, setLeadTime] = useState(0);
  const rules = useQuery({ queryKey: ['reorder-rules'], queryFn: getReorderRules });
  const suggestions = useQuery({
    queryKey: ['reorder-suggestions'],
    queryFn: getReorderSuggestions,
  });
  const locations = useQuery({
    queryKey: ['inventory-locations'],
    queryFn: () => getInventoryLocations({ limit: 100 }),
  });
  const products = useQuery({
    queryKey: ['products-reorder'],
    queryFn: () => getProducts({ limit: 500, sortBy: 'name', sortOrder: 'ASC' }),
  });
  const vendors = useQuery({
    queryKey: ['vendors-active'],
    queryFn: () => getVendors({ limit: 100 }),
  });
  const save = useMutation({
    mutationFn: () =>
      saveReorderRule({
        locationId,
        productId: product?.id ?? '',
        minimumPieces: minimum,
        targetPieces: target,
        preferredVendorId: vendorId || undefined,
        leadTimeDays: leadTime,
        isActive: true,
      }),
    onSuccess: () => {
      toastUtils.success('Reorder rule saved');
      void client.invalidateQueries({ queryKey: ['reorder-rules'] });
      void client.invalidateQueries({ queryKey: ['reorder-suggestions'] });
    },
    onError: (error: unknown) =>
      toastUtils.error(error instanceof Error ? error.message : 'Could not save rule'),
  });
  return (
    <OperationsShell
      header={{
        title: 'Reorder controls',
        description: 'Set location-specific minimums, targets, preferred vendors, and lead times.',
      }}
    >
      <Stack spacing={3}>
        {suggestions.data?.length ? (
          <Alert severity="warning">
            {suggestions.data.length} location/product combinations need replenishment.
          </Alert>
        ) : (
          <Alert severity="success">
            No configured reorder rules currently need replenishment.
          </Alert>
        )}
        {isAdmin ? (
          <Paper variant="outlined" sx={{ p: 2.5 }}>
            <Typography variant="h6" mb={2}>
              Add or update rule
            </Typography>
            <Stack direction={{ xs: 'column', lg: 'row' }} spacing={2}>
              <TextField
                select
                label="Location"
                value={locationId}
                onChange={(event) => setLocationId(event.target.value)}
                sx={{ minWidth: 190 }}
              >
                {locations.data?.data
                  .filter((item) => item.isActive)
                  .map((item) => (
                    <MenuItem key={item.id} value={item.id}>
                      {item.name}
                    </MenuItem>
                  ))}
              </TextField>
              <Autocomplete
                sx={{ minWidth: 230, flex: 1 }}
                options={products.data?.data ?? []}
                value={product}
                getOptionLabel={(item) => item.name}
                onChange={(_, item) => setProduct(item)}
                renderInput={(params) => <TextField {...params} label="Product" />}
              />
              <TextField
                label="Minimum pieces"
                type="number"
                value={minimum}
                onChange={(event) => setMinimum(Number(event.target.value))}
                sx={{ width: 150 }}
              />
              <TextField
                label="Target pieces"
                type="number"
                value={target}
                onChange={(event) => setTarget(Number(event.target.value))}
                sx={{ width: 150 }}
              />
              <TextField
                select
                label="Preferred vendor"
                value={vendorId}
                onChange={(event) => setVendorId(event.target.value)}
                sx={{ minWidth: 190 }}
              >
                <MenuItem value="">None</MenuItem>
                {vendors.data?.data
                  .filter((item) => item.isActive)
                  .map((item) => (
                    <MenuItem key={item.id} value={item.id}>
                      {item.name}
                    </MenuItem>
                  ))}
              </TextField>
              <TextField
                label="Lead days"
                type="number"
                value={leadTime}
                onChange={(event) => setLeadTime(Number(event.target.value))}
                sx={{ width: 120 }}
              />
              <Button
                variant="contained"
                disabled={
                  !locationId || !product || minimum < 0 || target < minimum || save.isPending
                }
                onClick={() => save.mutate()}
              >
                Save
              </Button>
            </Stack>
          </Paper>
        ) : null}
        <Paper variant="outlined" sx={{ overflowX: 'auto' }}>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>Product</TableCell>
                <TableCell>Location</TableCell>
                <TableCell align="right">Minimum</TableCell>
                <TableCell align="right">Target</TableCell>
                <TableCell align="right">Lead time</TableCell>
                <TableCell>Status</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {rules.data?.map((rule) => {
                const location = locations.data?.data.find((item) => item.id === rule.locationId);
                const suggested = suggestions.data?.find((item) => item.ruleId === rule.id);
                return (
                  <TableRow key={rule.id}>
                    <TableCell>{rule.productName}</TableCell>
                    <TableCell>{location?.name ?? rule.locationId.slice(0, 8)}</TableCell>
                    <TableCell align="right">{rule.minimumPieces}</TableCell>
                    <TableCell align="right">{rule.targetPieces}</TableCell>
                    <TableCell align="right">{rule.leadTimeDays} days</TableCell>
                    <TableCell>
                      <StatusBadge
                        label={suggested ? `Reorder ${suggested.suggestedPieces}` : 'Healthy'}
                        tone={suggested ? 'warning' : 'success'}
                      />
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Paper>
      </Stack>
    </OperationsShell>
  );
}
