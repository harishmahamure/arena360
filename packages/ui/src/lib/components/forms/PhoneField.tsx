'use client';

import { forwardRef } from 'react';
import { filterIntegerInput } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface PhoneFieldProps extends Omit<FormTextFieldProps, 'type'> {}

const PhoneField = forwardRef<HTMLDivElement, PhoneFieldProps>(
  ({ value, onChange, inputProps, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const filtered = filterIntegerInput(e.target.value, false);
          if (onChange) {
            e.target.value = filtered;
            onChange(e);
          }
        }}
        inputProps={{ inputMode: 'numeric', autoComplete: 'tel', ...inputProps }}
        {...props}
      />
    );
  },
);

PhoneField.displayName = 'PhoneField';

export default PhoneField;
