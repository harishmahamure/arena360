import { type Column, ListPage } from '@gaming-cafe/ui';
import { Box, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { StatsDateRangeToolbar } from '../../../containers/stats/StatsDateRangeToolbar';
import { useStatsDateRange } from '../../../hooks/useStatsDateRange';
import { getReceiptSummary, type ReceiptSummaryRow } from '../../../services/inventory';

type ReportRow = ReceiptSummaryRow & { id: string };

export default function InventoryReceiptReportPage() {
  const {
    startDate,
    endDate,
    apiFilters,
    setRange,
    applyPreset,
    setCompare,
    compare,
    isDirty,
    apply,
  } = useStatsDateRange();
  const [searchText, setSearchText] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['receipt-summary', apiFilters.startDate, apiFilters.endDate],
    queryFn: () =>
      getReceiptSummary({
        from: apiFilters.startDate,
        to: apiFilters.endDate,
      }),
  });

  const formatCurrency = (n: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(n);

  const rows: ReportRow[] = useMemo(() => {
    return (data ?? [])
      .map((r) => ({
        ...r,
        id: `${r.productId}-${r.vendorId ?? 'none'}`,
      }))
      .filter((r) =>
        `${r.productName} ${r.vendorName ?? ''}`.toLowerCase().includes(searchText.toLowerCase()),
      );
  }, [data, searchText]);

  const columns: Column<ReportRow>[] = [
    { id: 'productName', label: 'Product', minWidth: 160 },
    {
      id: 'vendorName',
      label: 'Vendor',
      minWidth: 140,
      format: (v) => (v as string) || '—',
    },
    {
      id: 'totalBoxes',
      label: 'Boxes',
      minWidth: 80,
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
    {
      id: 'totalPieces',
      label: 'Pieces',
      minWidth: 80,
      format: (v) => <Typography fontWeight={600}>{v as number}</Typography>,
    },
    {
      id: 'estimatedCost',
      label: 'Est. cost',
      minWidth: 120,
      format: (v) => formatCurrency(v as number),
    },
  ];

  const totalBoxes = rows.reduce((s, r) => s + r.totalBoxes, 0);
  const totalPieces = rows.reduce((s, r) => s + r.totalPieces, 0);
  const totalCost = rows.reduce((s, r) => s + r.estimatedCost, 0);

  return (
    <ListPage
      title="Receipt Report"
      description="Stock received into warehouse, aggregated by product and vendor"
      columns={columns}
      data={rows}
      actions={[]}
      isLoading={isLoading}
      showSearch
      searchValue={searchText}
      onSearchChange={(e) => setSearchText(e.target.value)}
      onSearchClear={() => setSearchText('')}
      filters={
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1, width: '100%' }}>
          <StatsDateRangeToolbar
            startDate={startDate}
            endDate={endDate}
            compare={compare}
            onRangeChange={setRange}
            onCompareChange={setCompare}
            onPreset={applyPreset}
            onApply={apply}
            isDirty={isDirty}
            showCompare={false}
          />
          <Typography variant="body2" color="text.secondary">
            Totals: {totalBoxes} boxes · {totalPieces} pieces · {formatCurrency(totalCost)}
          </Typography>
        </Box>
      }
    />
  );
}
