'use client';

import { Visibility, VisibilityOff } from '@mui/icons-material';
import { IconButton, InputAdornment, type TextFieldProps } from '@mui/material';
import { forwardRef, useState } from 'react';
import FormTextField from './FormTextField';

export interface PasswordFieldProps extends Omit<TextFieldProps, 'type' | 'InputProps'> {
  /**
   * If true, shows the toggle visibility icon
   * @default true
   */
  showToggle?: boolean;
  /**
   * Custom InputProps (will be merged with visibility toggle)
   */
  InputProps?: TextFieldProps['InputProps'];
}

/**
 * PasswordField - A password input field with optional visibility toggle
 * Automatically handles show/hide password functionality
 */
const PasswordField = forwardRef<HTMLDivElement, PasswordFieldProps>(
  ({ showToggle = true, InputProps, ...props }, ref) => {
    const [showPassword, setShowPassword] = useState(false);

    const togglePasswordVisibility = () => {
      setShowPassword((prev) => !prev);
    };

    const endAdornment = showToggle ? (
      <InputAdornment position="end">
        <IconButton
          onClick={togglePasswordVisibility}
          edge="end"
          aria-label="toggle password visibility"
        >
          {showPassword ? <VisibilityOff /> : <Visibility />}
        </IconButton>
      </InputAdornment>
    ) : (
      InputProps?.endAdornment
    );

    return (
      <FormTextField
        ref={ref}
        type={showPassword ? 'text' : 'password'}
        {...props}
        InputProps={{
          ...InputProps,
          endAdornment,
        }}
      />
    );
  },
);

PasswordField.displayName = 'PasswordField';

export default PasswordField;
