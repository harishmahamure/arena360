import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { formatCurrency } from '@gaming-cafe/utils';
import { Delete, Edit } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { deleteProduct } from '../../../services/product/delete';
import { getProducts, type ProductResponse } from '../../../services/product/list';

export default function ProductsPage() {
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const deletedProducts = searchParams.get('deleted') === 'true' ? 1 : 0;
  const stockExpiringSoon = searchParams.get('stockExpiringSoon') === 'true' ? 10 : undefined;

  const navigate = useNavigate();
  const { can } = usePermissions();
  const canWrite = can(Permission.ProductsWrite);

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleAddNewProduct = useCallback(() => {
    navigate('/products/new');
  }, [navigate]);

  const handleEditProduct = useCallback(
    (id: string) => {
      navigate(`/products/${id}`);
    },
    [navigate],
  );

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['products', debouncedSearch, page, deletedProducts, stockExpiringSoon],
    queryFn: () =>
      getProducts({
        name: debouncedSearch.length > 2 ? debouncedSearch : undefined,
        page: page,
        disabled: deletedProducts,
        ...(stockExpiringSoon && { stockQuantityLessThanTen: 10 }),
      }),
  });

  const handleSearch = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const query = event.target.value;
      setInputValue(query);
      debouncedSetSearch(query);
    },
    [debouncedSetSearch],
  );

  const handleClearSearch = useCallback(() => {
    setInputValue('');
    setDebouncedSearch('');
    debouncedSetSearch.clear();
  }, [debouncedSetSearch]);

  const columns: Column<ProductResponse>[] = [
    {
      id: 'name',
      label: 'Name',
      minWidth: 150,
    },
    {
      id: 'sku',
      label: 'SKU',
      minWidth: 100,
    },
    {
      id: 'price',
      label: 'Price',
      minWidth: 100,
      align: 'right',
      format: (value) => formatCurrency(parseFloat(value as string), 'INR'),
    },
    {
      id: 'stockQuantity',
      label: 'Stock',
      minWidth: 80,
      align: 'right',
    },
    {
      id: 'category',
      label: 'Category',
      minWidth: 80,
      format: (value) => (value as string).charAt(0).toUpperCase() + (value as string).slice(1),
    },
    {
      id: 'isActive',
      label: 'Status',
      minWidth: 80,
      align: 'center',
      format: (value) => (
        <Chip label={value ? 'Active' : 'Inactive'} color={value ? 'success' : 'error'} />
      ),
    },
  ];

  const handleDeactivateProduct = useCallback(
    async (id: string) => {
      try {
        await deleteProduct(id);
        toast.success('Product deactivated successfully');
        refetch();
      } catch (_error) {
        toast.error('Failed to deactivate product');
      }
    },
    [refetch],
  );

  const actions: Action<ProductResponse>[] = [
    {
      icon: <Edit color="info" />,
      label: 'Edit Product',
      onClick: (row) => handleEditProduct(row.id),
    },
    {
      icon: <Delete color="error" />,
      label: 'Deactivate Product',
      onClick: (row) => handleDeactivateProduct(row.id),
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<ProductResponse>
        title="Products"
        description="Manage your game zone Products here."
        data={data?.data || []}
        columns={columns}
        actions={canWrite ? actions : []}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={canWrite ? handleAddNewProduct : undefined}
        addButtonLabel="Add Product"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(value === 1 ? `/products` : `/products?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
