'use client';

import { forwardRef } from 'react';
import { filterIntegerInput } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

const OTP_MAX_LENGTH = 6;

export interface OtpFieldProps extends Omit<FormTextFieldProps, 'type'> {
  maxLength?: number;
}

const OtpField = forwardRef<HTMLDivElement, OtpFieldProps>(
  ({ value, onChange, inputProps, maxLength = OTP_MAX_LENGTH, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const stripped = e.target.value.replace(/\s+/g, '');
          const filtered = filterIntegerInput(stripped, false).slice(0, maxLength);
          if (onChange) {
            e.target.value = filtered;
            onChange(e);
          }
        }}
        inputProps={{
          inputMode: 'numeric',
          autoComplete: 'one-time-code',
          maxLength,
          ...inputProps,
        }}
        {...props}
      />
    );
  },
);

OtpField.displayName = 'OtpField';

export default OtpField;
