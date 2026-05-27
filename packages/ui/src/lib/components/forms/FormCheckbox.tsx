'use client';

import {
  Checkbox,
  type CheckboxProps,
  FormControlLabel,
  type FormControlLabelProps,
} from '@mui/material';
import { forwardRef } from 'react';

export interface FormCheckboxProps extends CheckboxProps {
  /**
   * Label for the checkbox
   */
  label?: React.ReactNode;
  /**
   * Props to pass to FormControlLabel
   */
  labelProps?: Omit<FormControlLabelProps, 'control' | 'label'>;
}

/**
 * FormCheckbox - A reusable checkbox component with label support
 * Wraps Material-UI Checkbox with FormControlLabel
 */
const FormCheckbox = forwardRef<HTMLButtonElement, FormCheckboxProps>(
  ({ label, labelProps, ...props }, ref) => {
    if (label) {
      return (
        <FormControlLabel
          control={<Checkbox ref={ref} {...props} />}
          label={label}
          {...labelProps}
        />
      );
    }

    return <Checkbox ref={ref} {...props} />;
  },
);

FormCheckbox.displayName = 'FormCheckbox';

export default FormCheckbox;
