import { ProductCategory as ProductCategoryValues } from '@gaming-cafe/contracts';
import type { FormSelectOption } from '@gaming-cafe/ui';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { Alert, TextField } from '@mui/material';
import { useEffect, useMemo, useRef, useState } from 'react';
import { type ControllerRenderProps, type UseFormReturn, useWatch } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import {
  type CreateProductFormData,
  createProductDefaultValues,
  createProductSchema,
  productCategoryOptions,
} from '../../../../src/containers/products/schemas/product-schema';
import {
  ProductStockQuantityField,
  ProductUnitConversionField,
} from '../../../containers/products/ProductUnitFields';
import { useProductUnits } from '../../../hooks/useProductUnits';
import { addProduct } from '../../../services/product/add';
import type { ProductCategory } from '../../../services/product/list';
import type { UnitResponse } from '../../../services/units/list';

function generateProductSku(name: string, category: string): string {
  const namePart = name
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
  if (!namePart) return '';
  const categoryPart =
    category === ProductCategoryValues.OTHER
      ? ''
      : category.toUpperCase().replace(/[^A-Z0-9]+/g, '-');
  return [namePart, categoryPart].filter(Boolean).join('-').slice(0, 100).replace(/-+$/g, '');
}

function AutoSkuField({
  field,
  form,
  disabled,
}: {
  field: ControllerRenderProps<CreateProductFormData, 'sku'>;
  form: UseFormReturn<CreateProductFormData>;
  disabled: boolean;
}) {
  const [name, category] = useWatch({ control: form.control, name: ['name', 'category'] });
  const manuallyEdited = useRef(false);

  useEffect(() => {
    if (!name?.trim() && !form.getValues('sku')) manuallyEdited.current = false;
    if (manuallyEdited.current) return;
    const generatedSku = generateProductSku(name ?? '', category ?? '');
    if (form.getValues('sku') !== generatedSku) {
      form.setValue('sku', generatedSku, { shouldDirty: false, shouldValidate: true });
    }
  }, [name, category, form]);

  return (
    <TextField
      {...field}
      inputRef={field.ref}
      fullWidth
      disabled={disabled}
      label="SKU"
      placeholder="Generated from product name and category"
      helperText={form.formState.errors.sku?.message || 'Generated as you type. Edit it any time.'}
      error={!!form.formState.errors.sku}
      value={field.value ?? ''}
      InputLabelProps={{ shrink: true }}
      onChange={(event) => {
        manuallyEdited.current = true;
        field.onChange(event.target.value);
      }}
    />
  );
}

function buildProductFormFields(
  unitSelectOptions: FormSelectOption[],
  units: UnitResponse[] = [],
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
      type: 'custom',
      hideCustomLabel: true,
      gridCols: 6,
      render: ({ field, form, disabled }) => (
        <AutoSkuField
          field={field as ControllerRenderProps<CreateProductFormData, 'sku'>}
          form={form}
          disabled={disabled}
        />
      ),
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
      type: 'currency',
      required: true,
      gridCols: 4,
      min: 0,
      helperText: 'Day price in ₹; charged during 8 AM – 11 PM venue time',
    },
    {
      name: 'purchasePricePerBox',
      label: 'Price per purchase unit (₹)',
      type: 'currency',
      gridCols: 4,
      min: 0,
      helperText: 'Cost per kg, box, or other selected purchase unit',
    },
    {
      name: 'unitsPerPurchaseUnit',
      label: 'Stock units per purchase unit',
      type: 'custom',
      hideCustomLabel: true,
      gridCols: 4,
      render: ({ field, form, disabled }) => (
        <ProductUnitConversionField
          field={field as ControllerRenderProps<CreateProductFormData, 'unitsPerPurchaseUnit'>}
          form={form}
          units={units}
          disabled={disabled}
        />
      ),
    },
    {
      name: 'unitId',
      label: 'Stock / recipe unit',
      type: 'select',
      gridCols: 4,
      options: unitSelectOptions,
      helperText:
        'Use grams for ingredients consumed in gram portions; pieces for individual items',
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
      label: 'Store stock (stock / recipe units)',
      type: 'custom',
      hideCustomLabel: true,
      gridCols: 6,
      render: ({ field, form, disabled }) => (
        <ProductStockQuantityField
          field={field as ControllerRenderProps<CreateProductFormData, 'stockQuantity'>}
          form={form}
          units={units}
          disabled={disabled}
        />
      ),
    },
    {
      name: 'isActive',
      label: 'Active (Available for sale)',
      type: 'switch',
      gridCols: 12,
      helperText: 'Inactive products are hidden from POS but kept in catalog',
    },
    {
      name: 'isRawMaterial',
      label: 'Raw material (ingredient only)',
      type: 'switch',
      gridCols: 12,
      helperText: 'Stocked and used in recipes, such as buns or patties, but never sold on its own',
    },
  ];
}

export function useProductFormFields(): FieldConfig<CreateProductFormData>[] {
  const { unitSelectOptions, units } = useProductUnits();
  return useMemo(
    () => buildProductFormFields(unitSelectOptions, units),
    [unitSelectOptions, units],
  );
}

export default function AddNewProductPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const [error, setError] = useState<string | undefined>();
  const { unitSelectOptions, units, defaultUnitIds, unitsReady, unitsMissing, unitsLoading } =
    useProductUnits();
  const productFormFields = useMemo(
    () => buildProductFormFields(unitSelectOptions, units),
    [unitSelectOptions, units],
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
        nightPrice: price,
        purchasePricePerBox: data.purchasePricePerBox ?? undefined,
        unitsPerPurchaseUnit: data.unitsPerPurchaseUnit ?? 1,
        unitId: data.unitId || undefined,
        purchaseUnitId: data.purchaseUnitId || undefined,
        category: category as ProductCategory,
        sku: data.sku ?? '',
        stockQuantity: data.stockQuantity || 0,
        isActive: data.isActive ?? true,
        isRawMaterial: data.isRawMaterial ?? false,
      });
      setTimeout(() => navigate('/products'), 1500);
    });
  };

  return (
    <FormPage
      title="Add New Product"
      description="Configure pricing, purchase units, and stock units"
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
        wizard
        wizardSteps={[
          { title: 'Product details', fields: ['name', 'sku', 'description', 'category'] },
          {
            title: 'Pricing & units',
            fields: [
              'price',
              'purchasePricePerBox',
              'unitsPerPurchaseUnit',
              'unitId',
              'purchaseUnitId',
            ],
          },
          {
            title: 'Stock & availability',
            fields: ['stockQuantity', 'isActive', 'isRawMaterial'],
          },
        ]}
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
