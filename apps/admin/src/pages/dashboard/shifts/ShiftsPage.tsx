import { type Column, ListViewPage } from '@gaming-cafe/ui';
import { Box, Chip, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { getShifts, type Shift } from '../../../services/shifts';
import { formatDisplayDateTime } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'warning' }> = {
  active: { label: 'Active', color: 'success' },
  completed: { label: 'Completed', color: 'default' },
  force_closed: { label: 'Force Closed', color: 'warning' },
};

export default function ShiftsPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');

  const { data, isLoading, error } = useQuery({
    queryKey: ['shifts', page],
    queryFn: () => getShifts({ page, sortBy: 'clockIn', sortOrder: 'DESC' }),
  });

  const columns: Column<Shift>[] = [
    {
      id: 'userId',
      label: 'Staff',
      minWidth: 150,
      format: (value) => `${(value as string).slice(0, 8)}...`,
    },
    {
      id: 'clockIn',
      label: 'Clock In',
      minWidth: 170,
      format: (value) => formatDisplayDateTime(value as string),
    },
    {
      id: 'clockOut',
      label: 'Clock Out',
      minWidth: 170,
      format: (value) => (value ? formatDisplayDateTime(value as string) : '-'),
    },
    {
      id: 'status',
      label: 'Status',
      minWidth: 120,
      format: (value) => {
        const config = statusConfig[value as string] || statusConfig.completed;
        return <Chip label={config.label} color={config.color} size="small" />;
      },
    },
  ];

  return (
    <Box>
      <ListViewPage
        title="Shifts"
        subtitle="Staff shift history"
        columns={columns}
        data={data?.data ?? []}
        actions={[{ label: 'View', onClick: (row) => navigate(`/shifts/${row.id}`) }]}
        isLoading={isLoading}
        error={error ? 'Failed to load shifts' : undefined}
      />
      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) => navigate(`/shifts?page=${p}`)}
          />
        </Box>
      )}
    </Box>
  );
}
