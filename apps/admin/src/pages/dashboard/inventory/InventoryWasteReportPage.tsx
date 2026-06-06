import { type Column, ListViewPage } from '@gaming-cafe/ui';
import { Box, TextField, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { getWasteSummary, type WasteSummaryRow } from '../../../services/inventory';

type ReportRow = WasteSummaryRow & { id: string };

export default function InventoryWasteReportPage() {
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [search, setSearch] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['waste-summary', from, to],
    queryFn: () =>
      getWasteSummary({
        ...(from ? { from: `${from}T00:00:00Z` } : {}),
        ...(to ? { to: `${to}T23:59:59Z` } : {}),
      }),
  });

  const formatCurrency = (n: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(n);

  const rows: ReportRow[] = useMemo(() => {
    return (data ?? [])
      .map((r) => ({
        ...r,
        id: `${r.locationId}-${r.productId}-${r.reasonCode}`,
      }))
      .filter((r) =>
        `${r.locationName} ${r.productName} ${r.reasonCode}`
          .toLowerCase()
          .includes(search.toLowerCase()),
      );
  }, [data, search]);

  const columns: Column<ReportRow>[] = [
    { id: 'locationName', label: 'Location', minWidth: 120 },
    { id: 'productName', label: 'Product', minWidth: 160 },
    {
      id: 'reasonCode',
      label: 'Reason',
      minWidth: 100,
      format: (v) => String(v).replace('_', ' '),
    },
    {
      id: 'totalPieces',
      label: 'Pieces wasted',
      minWidth: 100,
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
    {
      id: 'estimatedCost',
      label: 'Est. cost',
      minWidth: 120,
      format: (v) => formatCurrency(v as number),
    },
  ];

  const totalCost = rows.reduce((s, r) => s + r.estimatedCost, 0);
  const totalPieces = rows.reduce((s, r) => s + r.totalPieces, 0);

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <Box sx={{ display: 'flex', gap: 2, mb: 3, flexWrap: 'wrap', alignItems: 'center' }}>
        <TextField
          type="date"
          label="From"
          InputLabelProps={{ shrink: true }}
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          size="small"
        />
        <TextField
          type="date"
          label="To"
          InputLabelProps={{ shrink: true }}
          value={to}
          onChange={(e) => setTo(e.target.value)}
          size="small"
        />
        <Typography variant="body2" color="text.secondary" sx={{ ml: 'auto' }}>
          Total: {totalPieces} pcs · {formatCurrency(totalCost)}
        </Typography>
      </Box>

      <ListViewPage
        title="Waste Report"
        description="Approved waste aggregated by location, product, and reason"
        columns={columns}
        data={rows}
        actions={[]}
        isLoading={isLoading}
        inputValue={search}
        handleSearch={(e) => setSearch(e.target.value)}
        handleClearSearch={() => setSearch('')}
      />
    </Box>
  );
}
