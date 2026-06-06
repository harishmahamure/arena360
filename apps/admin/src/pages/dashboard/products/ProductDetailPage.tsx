import { FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductSchema,
} from '../../../containers/products/schemas/product-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getProductById } from '../../../services/product/getById';
import type { ProductCategory } from '../../../services/product/list';
import { updateProduct } from '../../../services/product/update';
import { productFormFieldsHook } from './ProductNewPage';

export default function EditProductPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const { can } = usePermissions();
  const canWrite = can(Permission.ProductsWrite);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const productFormFields = productFormFieldsHook();

  const { data: product, isLoading } = useQuery({
    queryKey: ['product', id],
    queryFn: () => getProductById(id as string),
    enabled: !!id,
  });

  const handleSubmit = async (data: CreateProductFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await updateProduct(id as string, {
        ...data,
        price: data.price,
        dayPrice: data.price,
        nightPrice: data.nightPrice ?? data.price,
      });
      setSuccess('Product updated successfully!');
      navigate('/products');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update product');
    } finally {
      setIsSubmitting(false);
    }
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  const dayPrice = product?.dayPrice ?? (product?.price ? parseFloat(product.price) : 0);

  return (
    <Paper elevation={0} sx={{ p: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Update Product
        </Typography>
        <Typography variant="body2" color="text.secondary">
          {product?.name}
        </Typography>
      </Box>

      <FormBuilder<CreateProductFormData>
        fields={productFormFields}
        schema={createProductSchema}
        defaultValues={{
          category: product?.category as unknown as ProductCategory,
          name: product?.name,
          price: dayPrice,
          nightPrice: product?.nightPrice ?? dayPrice,
          purchasePricePerBox: product?.purchasePricePerBox ?? undefined,
          unitsPerPurchaseUnit: product?.unitsPerPurchaseUnit ?? 1,
          unitId: product?.unitId ?? undefined,
          purchaseUnitId: product?.purchaseUnitId ?? undefined,
          sku: product?.sku,
          description: product?.description,
          stockQuantity: product?.stockQuantity,
          isActive: product?.isActive,
        }}
        mode={canWrite ? 'edit' : 'view'}
        onSubmit={handleSubmit}
        onCancel={() => navigate('/products')}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel={canWrite}
        showReset={canWrite}
        submitLabel="Update Product"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
