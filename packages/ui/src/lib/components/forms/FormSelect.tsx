'use client';

import {
  FormControl,
  FormHelperText,
  InputLabel,
  MenuItem,
  Select,
  type SelectProps,
} from '@mui/material';
import { forwardRef } from 'react';

export interface FormSelectOption {
  value: string | number;
  label: string;
  disabled?: boolean;
}

export interface FormSelectProps extends Omit<SelectProps, 'variant'> {
  /**
   * Label for the select field
   */
  label?: string;
  /**
   * Helper text to display below the select
   */
  helperText?: string;
  /**
   * If true, shows an error state
   */
  error?: boolean;
  /**
   * Options for the select dropdown
   */
  options?: FormSelectOption[];
  /**
   * Variant of the select field
   * @default "outlined"
   */
  variant?: SelectProps['variant'];
  /**
   * If true, the field is required
   */
  required?: boolean;
}

/**
 * FormSelect - A reusable select/dropdown component
 * Wraps Material-UI Select with consistent styling and helper text support
 */
const FormSelect = forwardRef<HTMLDivElement, FormSelectProps>(
  (
    {
      label,
      helperText,
      error = false,
      options = [],
      variant = 'outlined',
      required = false,
      fullWidth = true,
      ...props
    },
    ref,
  ) => {
    const labelId = label ? `${props.id || 'select'}-label` : undefined;

    return (
      <FormControl
        ref={ref}
        fullWidth={fullWidth}
        error={error}
        variant={variant}
        required={required}
      >
        {label && <InputLabel id={labelId}>{label}</InputLabel>}
        <Select label={label} labelId={labelId} {...props}>
          {options.map((option) => (
            <MenuItem key={option.value} value={option.value} disabled={option.disabled}>
              {option.label}
            </MenuItem>
          ))}
          {props.children}
        </Select>
        {helperText && <FormHelperText>{helperText}</FormHelperText>}
      </FormControl>
    );
  },
);

FormSelect.displayName = 'FormSelect';

export default FormSelect;
