import { type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Block, Visibility } from '@mui/icons-material';
import {
  Alert,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { usePermissions } from '../../../hooks/usePermissions';
import { useStaffNameMap } from '../../../hooks/useStaffNameMap';
import { forceCloseShift, getShifts, type Shift } from '../../../services/shifts';
import { buildListUrl } from '../../../utils/buildListUrl';
import { formatDisplayDateTime } from '../../../utils/date';

const statusConfig: Record<string, { label: string; color: 'success' | 'default' | 'warning' }> = {
  active: { label: 'Active', color: 'success' },
  completed: { label: 'Completed', color: 'default' },
  force_closed: { label: 'Force Closed', color: 'warning' },
};

export default function ShiftsPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { isAdmin } = usePermissions();
  const { resolveName } = useStaffNameMap();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');

  const [forceTarget, setForceTarget] = useState<Shift | null>(null);
  const [forcing, setForcing] = useState(false);

  const { data, isLoading, error } = useQuery({
    queryKey: ['shifts', page],
    queryFn: () => getShifts({ page, sortBy: 'clockIn', sortOrder: 'DESC' }),
  });

  const handleForceClose = async () => {
    if (!forceTarget) return;
    setForcing(true);
    try {
      await forceCloseShift(forceTarget.id);
      toastUtils.success('Shift force-closed');
      setForceTarget(null);
      void queryClient.invalidateQueries({ queryKey: ['shifts'] });
    } catch (err: unknown) {
      toastUtils.error(err instanceof Error ? err.message : 'Failed to force-close shift');
    } finally {
      setForcing(false);
    }
  };

  const columns: Column<Shift>[] = [
    {
      id: 'userId',
      label: 'Staff',
      minWidth: 150,
      format: (value) => resolveName(value as string),
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
        return (
          <Chip
            label={config?.label || 'Unknown'}
            color={config?.color || 'default'}
            size="small"
          />
        );
      },
    },
  ];

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load shifts
        </Alert>
      )}
      <ListPage<Shift>
        title="Shifts"
        description="Staff shift history"
        columns={columns}
        data={data?.data ?? []}
        actions={[
          {
            label: 'View',
            icon: <Visibility fontSize="small" />,
            onClick: (row) => navigate(`/shifts/${row.id}`),
          },
          ...(isAdmin
            ? [
                {
                  label: 'Force close',
                  icon: <Block fontSize="small" />,
                  color: 'warning' as const,
                  show: (row: Shift) => row.status === 'active',
                  onClick: (row: Shift) => setForceTarget(row),
                },
              ]
            : []),
        ]}
        isLoading={isLoading}
        showSearch={false}
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) => navigate(buildListUrl('/shifts', value, {})),
        }}
      />

      <Dialog open={forceTarget !== null} onClose={() => setForceTarget(null)}>
        <DialogTitle>Force close shift?</DialogTitle>
        <DialogContent>
          <DialogContentText>
            Force-close the active shift for {forceTarget ? resolveName(forceTarget.userId) : ''}?
            The staff member must start a new shift before operating the till or sessions.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setForceTarget(null)} disabled={forcing}>
            Cancel
          </Button>
          <Button
            onClick={() => void handleForceClose()}
            color="warning"
            variant="contained"
            disabled={forcing}
          >
            Force close
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
