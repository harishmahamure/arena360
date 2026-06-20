import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { formatCurrency } from '@gaming-cafe/utils';
import { Visibility } from '@mui/icons-material';
import {
  Alert,
  Box,
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
import { formatPaymentSplit } from '../../../containers/sales';
import { PaymentMethodValues } from '../../../containers/transactions/schemas/transaction-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { type CreditSettlementListRow, getCreditSettlements } from '../../../services/credit';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDateTime } from '../../../utils/date';

const paymentMethodLabel = (row: CreditSettlementListRow) => {
  if (row.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT) {
    return formatPaymentSplit({
      paymentMethod: row.paymentMethod,
      amount: row.amount,
      cashAmount: row.cashAmount,
      onlineAmount: row.onlineAmount,
    });
  }

  const labels: Record<string, string> = {
    [PaymentMethodValues.CASH]: 'Cash',
    [PaymentMethodValues.ONLINE]: 'Online',
    [PaymentMethodValues.CREDIT]: 'Credit',
  };

  return labels[row.paymentMethod] ?? row.paymentMethod;
};

export default function CreditSettlementsPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { can } = usePermissions();

  const page = Number(searchParams.get('page') || '1');
  const search = searchParams.get('search') || '';
  const paymentMethod = searchParams.get('paymentMethod') || '';
  const startDate = searchParams.get('startDate') || '';
  const endDate = searchParams.get('endDate') || '';

  const { data, isLoading, error } = useQuery({
    queryKey: ['credit-settlements', page, search, paymentMethod, startDate, endDate],
    queryFn: () =>
      getCreditSettlements({
        page,
        search: search || undefined,
        paymentMethod: paymentMethod || undefined,
        startDate: startDate || undefined,
        endDate: endDate || undefined,
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

  const columns: Column<CreditSettlementListRow>[] = [
    {
      id: 'settledAt',
      label: 'Settled',
      minWidth: 160,
      format: (value) => formatDisplayDateTime(value as string),
    },
    {
      id: 'playerUsername',
      label: 'Player',
      minWidth: 140,
    },
    {
      id: 'amount',
      label: 'Amount',
      minWidth: 110,
      align: 'right',
      format: (value) => (
        <Typography variant="body2" fontWeight={600}>
          {formatCurrency(Number(value), 'INR')}
        </Typography>
      ),
    },
    {
      id: 'paymentMethod',
      label: 'Payment',
      minWidth: 180,
      hideOnMobile: true,
      format: (_value, row) => (row ? paymentMethodLabel(row) : '—'),
    },
    {
      id: 'settledByUsername',
      label: 'Settled by',
      minWidth: 120,
      hideOnMobile: true,
    },
    {
      id: 'itemCount',
      label: 'Items',
      minWidth: 70,
      align: 'right',
      hideOnMobile: true,
    },
    {
      id: 'notes',
      label: 'Notes',
      minWidth: 160,
      hideOnMobile: true,
      format: (value) => {
        const text = (value as string | undefined) ?? '';
        return text.length > 48 ? `${text.slice(0, 48)}…` : text || '—';
      },
    },
  ];

  const actions: Action<CreditSettlementListRow>[] = [
    {
      label: 'View',
      icon: <Visibility />,
      onClick: (row) => navigate(`/credit/settlements/${row.id}`),
    },
  ];

  if (!can(Permission.CreditRead)) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>You do not have permission to view credit settlements.</Typography>
      </Box>
    );
  }

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load settlement history
        </Alert>
      )}

      <Stack
        direction={{ xs: 'column', md: 'row' }}
        spacing={2}
        sx={{ px: { xs: 2, md: 4 }, pt: { xs: 2, md: 3 }, pb: 1 }}
      >
        <TextField
          label="Search player"
          value={search}
          onChange={(e) => updateFilter('search', e.target.value)}
          size="small"
          sx={{ minWidth: 200 }}
        />
        <TextField
          label="From date"
          type="date"
          value={startDate}
          onChange={(e) => updateFilter('startDate', e.target.value)}
          size="small"
          InputLabelProps={{ shrink: true }}
        />
        <TextField
          label="To date"
          type="date"
          value={endDate}
          onChange={(e) => updateFilter('endDate', e.target.value)}
          size="small"
          InputLabelProps={{ shrink: true }}
        />
        <FormControl size="small" sx={{ minWidth: 160 }}>
          <InputLabel>Payment method</InputLabel>
          <Select
            label="Payment method"
            value={paymentMethod}
            onChange={(e) => updateFilter('paymentMethod', e.target.value)}
          >
            <MenuItem value="">All</MenuItem>
            <MenuItem value={PaymentMethodValues.CASH}>Cash</MenuItem>
            <MenuItem value={PaymentMethodValues.ONLINE}>Online</MenuItem>
            <MenuItem value={PaymentMethodValues.SPLIT_PAYMENT}>Split</MenuItem>
          </Select>
        </FormControl>
      </Stack>

      <ListPage<CreditSettlementListRow>
        title="Settlement history"
        description="Credit bills settled by staff, grouped by settlement date."
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        showSearch={false}
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(
              buildListUrl('/credit/settlements', value, {
                search,
                paymentMethod,
                startDate,
                endDate,
              }),
            ),
        }}
      />
    </>
  );
}
