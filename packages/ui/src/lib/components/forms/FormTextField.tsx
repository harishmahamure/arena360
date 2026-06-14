'use client';

import { TextField, type TextFieldProps } from '@mui/material';
import { forwardRef } from 'react';

export interface FormTextFieldProps extends Omit<TextFieldProps, 'variant'> {
  variant?: TextFieldProps['variant'];
}

const FormTextField = forwardRef<HTMLDivElement, FormTextFieldProps>(
  ({ variant = 'outlined', ...props }, ref) => {
    return (
      <TextField
        ref={ref}
        variant={variant}
        autoComplete="off"
        sx={{
          width: '100%',
          maxWidth: '100%',
          ...props.sx,
        }}
        {...props}
      />
    );
  },
);

FormTextField.displayName = 'FormTextField';

export default FormTextField;
