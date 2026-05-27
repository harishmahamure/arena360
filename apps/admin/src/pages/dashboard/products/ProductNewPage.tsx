import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductDefaultValues,
  createProductSchema,
  productCategoryOptions,
} from '../../../../src/containers/products/schemas/product-schema';
import { addProduct } from '../../../services/product/add';
import type { ProductCategory } from '../../../services/product/list';

export const productFormFields: FieldConfig<CreateProductFormData>[] = [
  {
    name: 'name',
    label: 'Product Name',
    type: 'text',
    placeholder: 'e.g., Coca Cola 500ml',
    required: true,
    gridCols: 6,
    helperText: 'Enter the product name (max 255 characters)',
  },
  {
    name: 'sku',
    label: 'SKU',
    type: 'text',
    required: false,
    placeholder: 'e.g., COCA-500',
    gridCols: 6,
    helperText: 'Stock Keeping Unit (optional, max 50 characters)',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Enter product description...',
    fullWidth: true,
    rows: 3,
    helperText: 'Optional product description',
  },
  {
    name: 'price',
    label: 'Price',
    type: 'number',
    placeholder: '0.00',
    required: true,
    gridCols: 4,
    min: 0,
    helperText: 'Product price (must be ≥ 0)',
  },
  {
    name: 'stockQuantity',
    label: 'Stock Quantity',
    type: 'number',
    placeholder: '0',
    gridCols: 4,
    min: 0,
    helperText: 'Available stock (optional)',
  },
  {
    name: 'category',
    label: 'Category',
    type: 'select',
    required: true,
    gridCols: 4,
    options: productCategoryOptions,
    helperText: 'Select product category',
  },
  {
    name: 'isActive',
    label: 'Active (Available for sale)',
    type: 'switch',
    gridCols: 12,
    helperText: 'Toggle to make product available for sale',
  },
];

export default function AddNewProductPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreateProductFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.name || !data.price || !data.category) {
      setError('All fields are required');
      setLoading(false);
      return;
    }
    try {
      await addProduct({
        name: data.name,
        description: data.description || '',
        price: data.price,
        category: data.category as ProductCategory,
        sku: data.sku || '',
        stockQuantity: data.stockQuantity || 0,
        isActive: data.isActive ?? true,
      });

      // Example API call:
      // await http.post('/products', data);

      setSuccess('Product created successfully!');

      // Navigate back to products list after a short delay
      setTimeout(() => {
        navigate('/products');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create product');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/products');
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Product
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Fill in the details below to create a new product
        </Typography>
      </Box>

      <FormBuilder<CreateProductFormData>
        fields={productFormFields}
        schema={createProductSchema}
        defaultValues={createProductDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
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
