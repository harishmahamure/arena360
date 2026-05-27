import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { Delete, Edit } from '@mui/icons-material';
import { Alert, Box, Chip, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteVendor, getVendors, type Vendor } from '../../../services/vendors';
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
    {
      label: 'Edit',
      icon: <Edit fontSize="small" />,
      onClick: (row) => navigate(`/vendors/${row.id}`),
    },
    ...(can(Permission.VendorsWrite)
      ? [
          {
            label: 'Delete',
            icon: <Delete fontSize="small" />,
            color: 'error' as const,
            onClick: (row: Vendor) => handleDelete(row),
          },
        ]
      : []),
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          Failed to load vendors
        </Alert>
      )}
      <ListViewPage<Vendor>
        title="Vendors"
        description="Manage suppliers and vendors"
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        inputValue=""
        handleSearch={() => {}}
        handleClearSearch={() => {}}
        onAddClick={can(Permission.VendorsWrite) ? () => navigate('/vendors/new') : undefined}
        addButtonLabel="Add Vendor"
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
