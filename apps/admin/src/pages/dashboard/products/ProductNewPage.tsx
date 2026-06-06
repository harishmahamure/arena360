import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductDefaultValues,
  createProductSchema,
  productCategoryOptions,
} from '../../../../src/containers/products/schemas/product-schema';
import { addProduct } from '../../../services/product/add';
import type { ProductCategory } from '../../../services/product/list';
import { getUnits } from '../../../services/units/list';

function useProductFormFields(): FieldConfig<CreateProductFormData>[] {
  const { data: unitsData } = useQuery({
    queryKey: ['units-for-product'],
    queryFn: () => getUnits({ limit: 100 }),
  });

  const unitOptions = useMemo(
    () =>
      (unitsData?.data ?? []).map((u) => ({
        value: u.id,
        label: `${u.name} (${u.abbreviation})`,
      })),
    [unitsData],
  );

  return useMemo(
    (): FieldConfig<CreateProductFormData>[] => [
      {
        name: 'name',
        label: 'Product Name',
        type: 'text',
        placeholder: 'e.g., Coca Cola 500ml',
        required: true,
        gridCols: 6,
      },
      {
        name: 'sku',
        label: 'SKU',
        type: 'text',
        placeholder: 'e.g., COCA-500',
        gridCols: 6,
      },
      {
        name: 'description',
        label: 'Description',
        type: 'textarea',
        fullWidth: true,
        rows: 2,
      },
      {
        name: 'price',
        label: 'Day price (₹)',
        type: 'number',
        required: true,
        gridCols: 4,
        min: 0,
      },
      {
        name: 'nightPrice',
        label: 'Night price (₹)',
        type: 'number',
        required: true,
        gridCols: 4,
        min: 0,
        helperText: 'Used 11 PM – 8 AM venue time',
      },
      {
        name: 'purchasePricePerBox',
        label: 'Purchase price / box (₹)',
        type: 'number',
        gridCols: 4,
        min: 0,
      },
      {
        name: 'unitsPerPurchaseUnit',
        label: 'Units per box',
        type: 'number',
        gridCols: 4,
        min: 1,
        helperText: 'Pieces in one purchase unit (box)',
      },
      {
        name: 'unitId',
        label: 'Sale unit',
        type: 'select',
        gridCols: 4,
        options: [{ value: '', label: 'Default (piece)' }, ...unitOptions],
      },
      {
        name: 'purchaseUnitId',
        label: 'Purchase unit',
        type: 'select',
        gridCols: 4,
        options: [{ value: '', label: 'Default (box)' }, ...unitOptions],
      },
      {
        name: 'category',
        label: 'Category',
        type: 'select',
        required: true,
        gridCols: 4,
        options: productCategoryOptions,
      },
      {
        name: 'stockQuantity',
        label: 'Initial store stock (pieces)',
        type: 'number',
        gridCols: 4,
        min: 0,
        helperText: 'Optional; synced to default store on create',
      },
      {
        name: 'isActive',
        label: 'Active (Available for sale)',
        type: 'switch',
        gridCols: 12,
      },
    ],
    [unitOptions],
  );
}

export default function AddNewProductPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const productFormFields = useProductFormFields();

  const handleSubmit = async (data: CreateProductFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.name || data.price == null || !data.category) {
      setError('Name, day price, and category are required');
      setLoading(false);
      return;
    }
    try {
      await addProduct({
        name: data.name,
        description: data.description || '',
        price: data.price,
        dayPrice: data.price,
        nightPrice: data.nightPrice ?? data.price,
        purchasePricePerBox: data.purchasePricePerBox ?? undefined,
        unitsPerPurchaseUnit: data.unitsPerPurchaseUnit ?? 1,
        unitId: data.unitId || undefined,
        purchaseUnitId: data.purchaseUnitId || undefined,
        category: data.category as ProductCategory,
        sku: data.sku || '',
        stockQuantity: data.stockQuantity || 0,
        isActive: data.isActive ?? true,
      });
      setSuccess('Product created successfully!');
      setTimeout(() => navigate('/products'), 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create product');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Paper elevation={0} sx={{ p: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Product
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Configure pricing, box units, and sale units
        </Typography>
      </Box>

      <FormBuilder<CreateProductFormData>
        fields={productFormFields}
        schema={createProductSchema}
        defaultValues={createProductDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={() => navigate('/products')}
        loading={loading}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Create Product"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}

export { useProductFormFields as productFormFieldsHook };
