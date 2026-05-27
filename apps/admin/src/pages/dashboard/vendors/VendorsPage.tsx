import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Add } from '@mui/icons-material';
import { Box, Button, Chip, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteVendor, getVendors, type Vendor } from '../../../services/expenses';
import { formatDisplayDate } from '../../../utils/date';

export default function VendorsPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page') || '1');
  const { can } = usePermissions();

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['vendors', page],
    queryFn: () => getVendors({ page, limit: 20 }),
  });

  const handleDelete = async (vendor: Vendor) => {
    if (!confirm(`Delete vendor "${vendor.name}"?`)) return;
    try {
      await deleteVendor(vendor.id);
      toast.success('Vendor deleted');
      refetch();
    } catch {
      toast.error('Failed to delete vendor');
    }
  };

  const columns: Column<Vendor>[] = [
    { id: 'name', label: 'Name', minWidth: 200 },
    {
      id: 'contactPerson',
      label: 'Contact',
      minWidth: 150,
      format: (value) => (value as string) || '-',
    },
    {
      id: 'phone',
      label: 'Phone',
      minWidth: 130,
      format: (value) => (value as string) || '-',
    },
    {
      id: 'gstNumber',
      label: 'GST',
      minWidth: 150,
      format: (value) => (value as string) || '-',
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 100,
      format: (value) => (
        <Chip
          label={value ? 'Active' : 'Inactive'}
          color={value ? 'success' : 'error'}
          size="small"
        />
      ),
    },
    {
      id: 'createdAt',
      label: 'Added',
      minWidth: 120,
      format: (value) => formatDisplayDate(value as string),
    },
  ];

  const actions: Action<Vendor>[] = [
    { label: 'Edit', onClick: (row) => navigate(`/vendors/${row.id}`) },
    ...(can(Permission.VendorsWrite)
      ? [{ label: 'Delete', onClick: (row: Vendor) => handleDelete(row) }]
      : []),
  ];

  return (
    <Box>
      <ListViewPage
        title="Vendors"
        subtitle="Manage suppliers and vendors"
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        error={error ? 'Failed to load vendors' : undefined}
        headerAction={
          can(Permission.VendorsWrite) ? (
            <Button
              variant="contained"
              startIcon={<Add />}
              onClick={() => navigate('/vendors/new')}
            >
              Add Vendor
            </Button>
          ) : undefined
        }
      />
      {data && data.totalPages > 1 && (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
          <Pagination
            count={data.totalPages}
            page={page}
            onChange={(_, p) => navigate(`/vendors?page=${p}`)}
          />
        </Box>
      )}
    </Box>
  );
}
