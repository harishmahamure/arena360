import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { Delete, Edit } from '@mui/icons-material';
import { Alert, Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteVendor, getVendors, type Vendor } from '../../../services/vendors';
import { buildListUrl } from '../../../utils/buildListUrl';
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
      toastUtils.success('Vendor deleted');
      refetch();
    } catch {
      toastUtils.error('Failed to delete vendor');
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
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2, mx: { xs: 2, md: 4 }, mt: { xs: 2, md: 3 } }}>
          Failed to load vendors
        </Alert>
      )}
      <ListPage<Vendor>
        title="Vendors"
        description="Manage suppliers and vendors"
        columns={columns}
        data={data?.data ?? []}
        actions={actions}
        isLoading={isLoading}
        showSearch={false}
        onAddClick={can(Permission.VendorsWrite) ? () => navigate('/vendors/new') : undefined}
        addButtonLabel="Add Vendor"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) => navigate(buildListUrl('/vendors', value, {})),
        }}
      />
    </>
  );
}
