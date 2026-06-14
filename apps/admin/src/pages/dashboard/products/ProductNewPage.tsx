import type { FormSelectOption } from '@gaming-cafe/ui';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { Alert } from '@mui/material';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductDefaultValues,
  createProductSchema,
  productCategoryOptions,
} from '../../../../src/containers/products/schemas/product-schema';
import { useProductUnits } from '../../../hooks/useProductUnits';
import { addProduct } from '../../../services/product/add';
import type { ProductCategory } from '../../../services/product/list';

function buildProductFormFields(
  unitSelectOptions: FormSelectOption[],
): FieldConfig<CreateProductFormData>[] {
  return [
    {
      name: 'name',
      label: 'Product Name',
      type: 'text',
      placeholder: 'e.g., Coca Cola 500ml',
      required: true,
      gridCols: 6,
      helperText: 'Shown on receipts and the POS product grid',
    },
    {
      name: 'sku',
      label: 'SKU',
      type: 'text',
      placeholder: 'e.g., COCA-500',
      gridCols: 6,
      helperText: 'Optional stock-keeping unit for inventory tracking',
    },
    {
      name: 'description',
      label: 'Description',
      type: 'textarea',
      fullWidth: true,
      rows: 2,
      helperText: 'Optional short description shown on POS cards',
    },
    {
      name: 'price',
      label: 'Day price (₹)',
      type: 'number',
      required: true,
      gridCols: 4,
      min: 0,
      helperText: 'Day price in ₹; charged during 8 AM – 11 PM venue time',
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
      helperText: 'Cost per purchase unit (box); used for margin reporting',
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
      options: unitSelectOptions,
      helperText: 'Unit sold to players at POS',
    },
    {
      name: 'purchaseUnitId',
      label: 'Purchase unit',
      type: 'select',
      gridCols: 4,
      options: unitSelectOptions,
      helperText: 'Unit used when receiving stock from vendors',
    },
    {
      name: 'category',
      label: 'Category',
      type: 'select',
      required: true,
      gridCols: 4,
      options: productCategoryOptions,
      helperText: 'Product category for reporting and filtering',
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
      helperText: 'Inactive products are hidden from POS but kept in catalog',
    },
  ];
}

export function useProductFormFields(): FieldConfig<CreateProductFormData>[] {
  const { unitSelectOptions } = useProductUnits();
  return useMemo(() => buildProductFormFields(unitSelectOptions), [unitSelectOptions]);
}

export default function AddNewProductPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const [error, setError] = useState<string | undefined>();
  const { unitSelectOptions, defaultUnitIds, unitsReady, unitsMissing, unitsLoading } =
    useProductUnits();
  const productFormFields = useMemo(
    () => buildProductFormFields(unitSelectOptions),
    [unitSelectOptions],
  );

  const defaultValues = useMemo(
    (): CreateProductFormData => ({
      ...createProductDefaultValues,
      unitId: defaultUnitIds.sale,
      purchaseUnitId: defaultUnitIds.purchase,
    }),
    [defaultUnitIds],
  );

  const handleSubmit = async (data: CreateProductFormData) => {
    setError(undefined);
    if (!data.name || data.price == null || !data.category) {
      setError('Name, day price, and category are required');
      return;
    }
    const { name, price, category } = data;
    void run(async () => {
      await addProduct({
        name,
        description: data.description || '',
        price,
        dayPrice: price,
        nightPrice: data.nightPrice ?? price,
        purchasePricePerBox: data.purchasePricePerBox ?? undefined,
        unitsPerPurchaseUnit: data.unitsPerPurchaseUnit ?? 1,
        unitId: data.unitId || undefined,
        purchaseUnitId: data.purchaseUnitId || undefined,
        category: category as ProductCategory,
        sku: data.sku || '',
        stockQuantity: data.stockQuantity || 0,
        isActive: data.isActive ?? true,
      });
      setTimeout(() => navigate('/products'), 1500);
    });
  };

  return (
    <FormPage
      title="Add New Product"
      description="Configure pricing, box units, and sale units"
      backTo="/products"
      backLabel="Back to products"
      breadcrumbs={[{ label: 'Products', to: '/products' }, { label: 'New product' }]}
    >
      {unitsMissing ? (
        <Alert severity="warning" sx={{ mb: 2 }}>
          Product units are not available yet. Refresh the page or contact an administrator if sale
          and purchase unit dropdowns stay empty.
        </Alert>
      ) : null}
      <FormBuilder<CreateProductFormData>
        key={unitsReady ? 'units-ready' : unitsLoading ? 'units-loading' : 'units-empty'}
        fields={productFormFields}
        schema={createProductSchema}
        defaultValues={defaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={() => navigate('/products')}
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Product created"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to create product'}
        error={error}
        showCancel
        showReset
        submitLabel="Create Product"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </FormPage>
  );
}

export { useProductFormFields as productFormFieldsHook };
