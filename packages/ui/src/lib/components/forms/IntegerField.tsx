'use client';

import { forwardRef } from 'react';
import { filterIntegerInput } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface IntegerFieldProps extends Omit<FormTextFieldProps, 'type'> {
  allowNegative?: boolean;
}

const IntegerField = forwardRef<HTMLDivElement, IntegerFieldProps>(
  ({ value, onChange, inputProps, allowNegative = false, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const filtered = filterIntegerInput(e.target.value, allowNegative);
          if (onChange) {
            e.target.value = filtered;
            onChange(e);
          }
        }}
        inputProps={{ inputMode: 'numeric', ...inputProps }}
        {...props}
      />
    );
  },
);

IntegerField.displayName = 'IntegerField';

export default IntegerField;
