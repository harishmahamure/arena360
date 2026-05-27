import { FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductSchema,
} from '../../../containers/products/schemas/product-schema';
import { getProductById } from '../../../services/product/getById';
import type { ProductCategory } from '../../../services/product/list';
import { updateProduct } from '../../../services/product/update';
import { productFormFields } from './ProductNewPage';

export default function EditProductPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: product, isLoading } = useQuery({
    queryKey: ['product', id],
    queryFn: () => getProductById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30, // 30 seconds
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
    refetchInterval: 1000 * 30,
    refetchIntervalInBackground: true,
  });

  const handleSubmit = async (data: CreateProductFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.name || !data.price || !data.category) {
      setError('All fields are required');
      setIsSubmitting(false);
      return;
    }
    try {
      await updateProduct(id as string, {
        name: data.name,
        description: data.description,
        price: data.price,
        category: data.category,
        sku: data.sku,
        stockQuantity: data.stockQuantity,
        isActive: data.isActive,
      });
      setSuccess('Product updated successfully!');
      navigate('/products');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update product');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/products');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Update Product{' '}
        </Typography>
        <Typography variant="body2" color="text.secondary">
          {product?.name} to update the product details.
        </Typography>
      </Box>

      <FormBuilder<CreateProductFormData>
        fields={productFormFields}
        schema={createProductSchema}
        defaultValues={{
          category: product?.category as unknown as ProductCategory,
          name: product?.name,
          price: product?.price ? parseFloat(product?.price) : 0,
          sku: product?.sku,
          description: product?.description,
          stockQuantity: product?.stockQuantity,
          isActive: product?.isActive,
        }}
        mode="edit"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Update Product"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
