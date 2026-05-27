'use client';

import {
  FormControlLabel,
  type FormControlLabelProps,
  Switch,
  type SwitchProps,
} from '@mui/material';
import { forwardRef } from 'react';

export interface FormSwitchProps extends SwitchProps {
  /**
   * Label for the switch
   */
  label?: React.ReactNode;
  /**
   * Props to pass to FormControlLabel
   */
  labelProps?: Omit<FormControlLabelProps, 'control' | 'label'>;
}

/**
 * FormSwitch - A reusable switch component with label support
 * Wraps Material-UI Switch with FormControlLabel
 */
const FormSwitch = forwardRef<HTMLButtonElement, FormSwitchProps>(
  ({ label, labelProps, ...props }, ref) => {
    if (label) {
      return (
        <FormControlLabel control={<Switch ref={ref} {...props} />} label={label} {...labelProps} />
      );
    }

    return <Switch ref={ref} {...props} />;
  },
);

FormSwitch.displayName = 'FormSwitch';

export default FormSwitch;
