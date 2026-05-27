import { type Column, ListViewPage } from '@gaming-cafe/ui';
import { Box, Chip, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { type CashRegister, getCashRegisters } from '../../../services/cash-registers';
import { formatDisplayDateTime } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'info' }> = {
  open: { label: 'Open', color: 'success' },
  closed: { label: 'Closed', color: 'default' },
  reconciled: { label: 'Reconciled', color: 'info' },
};

export default function CashRegistersPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  const { data, isLoading, error } = useQuery({
    queryKey: ['cash-registers', page],
    queryFn: () => getCashRegisters({ page }),
  });

  const columns: Column<CashRegister>[] = [
    {
      id: 'openingBalance',
      label: 'Opening',
      minWidth: 120,
      format: (value) => (
        <Typography variant="body2" fontWeight={600}>
          {formatCurrency(value as number)}
        </Typography>
      ),
    },
    {
      id: 'closingBalance',
      label: 'Closing',
      minWidth: 120,
      format: (value) =>
        value != null ? (
          <Typography variant="body2" fontWeight={600}>
            {formatCurrency(value as number)}
          </Typography>
        ) : (
          '-'
        ),
    },
    {
      id: 'variance',
      label: 'Variance',
      minWidth: 120,
      format: (value) => {
        if (value == null) return '-';
        const v = value as number;
        return (
          <Typography
            variant="body2"
            fontWeight={600}
            color={v === 0 ? 'success.main' : 'error.main'}
          >
            {formatCurrency(v)}
          </Typography>
        );
      },
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 110,
      format: (value) => {
        const config = statusConfig[value as string] || statusConfig.closed;
        return <Chip label={config.label} color={config.color} size="small" />;
      },
    },
    {
      id: 'createdAt',
      label: 'Opened At',
      minWidth: 170,
      format: (value) => formatDisplayDateTime(value as string),
    },
  ];

  return (
    <Box>
      <ListViewPage
        title="Cash Registers"
        subtitle="Cash register history and reconciliation"
        columns={columns}
        data={data?.data ?? []}
        actions={[{ label: 'View', onClick: (row) => navigate(`/cash-registers/${row.id}`) }]}
        isLoading={isLoading}
        error={error ? 'Failed to load cash registers' : undefined}
      />
      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) => navigate(`/cash-registers?page=${p}`)}
          />
        </Box>
      )}
    </Box>
  );
}
