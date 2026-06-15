'use client';

import { sanitizeUsernameInput } from '@gaming-cafe/utils';
import { forwardRef } from 'react';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface UsernameFieldProps extends Omit<FormTextFieldProps, 'type'> {}

const UsernameField = forwardRef<HTMLDivElement, UsernameFieldProps>(
  ({ value, onChange, inputProps, ...props }, ref) => {
    return (
      <FormTextField
        ref={ref}
        type="text"
        value={value ?? ''}
        onChange={(e) => {
          const normalized = sanitizeUsernameInput(e.target.value);
          if (onChange) {
            e.target.value = normalized;
            onChange(e);
          }
        }}
        inputProps={{ autoComplete: 'username', ...inputProps }}
        {...props}
      />
    );
  },
);

UsernameField.displayName = 'UsernameField';

export default UsernameField;
