'use client';

import {
  FormControl,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  Radio,
  RadioGroup,
  type RadioGroupProps,
} from '@mui/material';
import { forwardRef } from 'react';

export interface FormRadioOption {
  value: string | number;
  label: string;
  disabled?: boolean;
}

export interface FormRadioGroupProps extends RadioGroupProps {
  /**
   * Label for the radio group
   */
  label?: string;
  /**
   * Helper text to display below the radio group
   */
  helperText?: string;
  /**
   * If true, shows an error state
   */
  error?: boolean;
  /**
   * Options for the radio group
   */
  options?: FormRadioOption[];
  /**
   * If true, the field is required
   */
  required?: boolean;
}

/**
 * FormRadioGroup - A reusable radio button group component
 * Wraps Material-UI RadioGroup with consistent styling and helper text support
 */
const FormRadioGroup = forwardRef<HTMLDivElement, FormRadioGroupProps>(
  ({ label, helperText, error = false, options = [], required = false, ...props }, ref) => {
    return (
      <FormControl ref={ref} error={error} required={required}>
        {label && <FormLabel>{label}</FormLabel>}
        <RadioGroup {...props}>
          {options.map((option) => (
            <FormControlLabel
              key={option.value}
              value={option.value}
              control={<Radio />}
              label={option.label}
              disabled={option.disabled}
            />
          ))}
          {props.children}
        </RadioGroup>
        {helperText && <FormHelperText>{helperText}</FormHelperText>}
      </FormControl>
    );
  },
);

FormRadioGroup.displayName = 'FormRadioGroup';

export default FormRadioGroup;
