'use client';

import { InputAdornment } from '@mui/material';
import { forwardRef } from 'react';
import { filterCurrencyInput } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface CurrencyFieldProps extends Omit<FormTextFieldProps, 'type'> {
  currencySymbol?: string;
}

const CurrencyField = forwardRef<HTMLDivElement, CurrencyFieldProps>(
  ({ currencySymbol = '₹', value, onChange, inputProps, InputProps, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const filtered = filterCurrencyInput(e.target.value);
          if (onChange) {
            e.target.value = filtered;
            onChange(e);
          }
        }}
        inputProps={{ inputMode: 'decimal', ...inputProps }}
        InputProps={{
          startAdornment: <InputAdornment position="start">{currencySymbol}</InputAdornment>,
          ...InputProps,
        }}
        {...props}
      />
    );
  },
);

CurrencyField.displayName = 'CurrencyField';

export default CurrencyField;
