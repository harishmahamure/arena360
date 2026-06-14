'use client';

import { forwardRef } from 'react';
import { filterDecimalInput } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface DecimalFieldProps extends Omit<FormTextFieldProps, 'type'> {
  decimalPlaces?: number;
  allowNegative?: boolean;
}

const DecimalField = forwardRef<HTMLDivElement, DecimalFieldProps>(
  ({ value, onChange, inputProps, decimalPlaces = 2, allowNegative = false, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const filtered = filterDecimalInput(e.target.value, decimalPlaces, allowNegative);
          if (onChange) {
            e.target.value = filtered;
            onChange(e);
          }
        }}
        inputProps={{ inputMode: 'decimal', ...inputProps }}
        {...props}
      />
    );
  },
);

DecimalField.displayName = 'DecimalField';

export default DecimalField;
