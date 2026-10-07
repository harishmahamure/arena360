import { DecimalField, IntegerField } from '@gaming-cafe/ui';
import { MenuItem, Stack, TextField } from '@mui/material';
import { useEffect, useState } from 'react';
import { type ControllerRenderProps, type UseFormReturn, useWatch } from 'react-hook-form';
import type { UnitResponse } from '../../services/units/list';
import { metricUnitConversion } from './productUnits';
import type { CreateProductFormData } from './schemas/product-schema';

type UnitFieldProps<Name extends 'unitsPerPurchaseUnit' | 'stockQuantity'> = {
  field: ControllerRenderProps<CreateProductFormData, Name>;
  form: UseFormReturn<CreateProductFormData>;
  units: UnitResponse[];
  disabled: boolean;
};

function useSelectedUnits(form: UseFormReturn<CreateProductFormData>, units: UnitResponse[]) {
  const [stockId, purchaseId] = useWatch({
    control: form.control,
    name: ['unitId', 'purchaseUnitId'],
  });
  const stock = units.find((unit) => unit.id === stockId);
  const purchase = units.find((unit) => unit.id === purchaseId);
  return { stock, purchase, conversion: metricUnitConversion(purchase?.type, stock?.type) };
}

export function ProductUnitConversionField({
  field,
  form,
  units,
  disabled,
}: UnitFieldProps<'unitsPerPurchaseUnit'>) {
  const { stock, purchase, conversion } = useSelectedUnits(form, units);
  useEffect(() => {
    if (
      !disabled &&
      conversion !== undefined &&
      form.getValues('unitsPerPurchaseUnit') !== conversion
    ) {
      form.setValue('unitsPerPurchaseUnit', conversion, {
        shouldDirty: true,
        shouldValidate: true,
      });
    }
  }, [conversion, disabled, form]);
  const error = form.formState.errors.unitsPerPurchaseUnit?.message;
  const helper =
    conversion !== undefined
      ? conversion < 1
        ? 'Use a smaller stock unit, such as grams or milliliters, to track recipe portions.'
        : `1 ${purchase?.abbreviation} = ${conversion.toLocaleString()} ${stock?.abbreviation}. Conversion is automatic.`
      : 'Stock units in one purchase unit (for example, 12 pieces per box).';

  return (
    <IntegerField
      {...field}
      inputRef={field.ref}
      label="Stock units per purchase unit"
      value={field.value ?? 1}
      disabled={disabled}
      fullWidth
      InputLabelProps={{ shrink: true }}
      InputProps={{ readOnly: conversion !== undefined }}
      error={!!error || (conversion !== undefined && conversion < 1)}
      helperText={error || helper}
      onChange={(event) =>
        field.onChange(event.target.value === '' ? '' : Number(event.target.value))
      }
    />
  );
}

export function ProductStockQuantityField({
  field,
  form,
  units,
  disabled,
}: UnitFieldProps<'stockQuantity'>) {
  const { stock, purchase, conversion } = useSelectedUnits(form, units);
  const [entryUnit, setEntryUnit] = useState('purchase');
  const convertible = conversion !== undefined && conversion > 1;
  const factor = convertible && entryUnit === 'purchase' ? conversion : 1;
  const unit = convertible && entryUnit === 'purchase' ? purchase : stock;
  const abbreviation = unit?.abbreviation ?? 'units';
  const error = form.formState.errors.stockQuantity?.message;

  return (
    <Stack direction="row" spacing={1} alignItems="flex-start">
      <DecimalField
        name={field.name}
        inputRef={field.ref}
        label={`Store stock (${abbreviation})`}
        value={field.value == null ? '' : field.value / factor}
        decimalPlaces={factor === 1 ? 0 : 3}
        commitMode="blur"
        disabled={disabled}
        fullWidth
        InputLabelProps={{ shrink: true }}
        error={!!error}
        helperText={
          error ||
          (convertible
            ? `${(field.value ?? 0).toLocaleString()} ${stock?.abbreviation} in stock. Recipes use ${stock?.abbreviation}.`
            : 'Stock at the default store, in the stock / recipe unit.')
        }
        onBlur={field.onBlur}
        onChange={(event) =>
          field.onChange(
            event.target.value === '' ? undefined : Math.round(Number(event.target.value) * factor),
          )
        }
      />
      {convertible && (
        <TextField
          select
          label="Entry unit"
          value={entryUnit}
          onChange={(event) => setEntryUnit(event.target.value)}
          disabled={disabled}
          sx={{ minWidth: 100 }}
        >
          <MenuItem value="purchase">{purchase?.abbreviation}</MenuItem>
          <MenuItem value="stock">{stock?.abbreviation}</MenuItem>
        </TextField>
      )}
    </Stack>
  );
}
