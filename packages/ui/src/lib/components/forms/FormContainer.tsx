'use client';

import { Box, type BoxProps } from '@mui/material';
import { forwardRef } from 'react';

export interface FormContainerProps extends Omit<BoxProps, 'component' | 'onSubmit'> {
  /**
   * Form submission handler
   */
  onSubmit?: (e: React.FormEvent<HTMLFormElement>) => void;
  /**
   * If true, prevents default form submission behavior
   * @default true
   */
  preventDefault?: boolean;
}

/**
 * FormContainer - A wrapper component for forms
 * Provides consistent form styling and handles form submission
 */
const FormContainer = forwardRef<HTMLFormElement, FormContainerProps>(
  ({ onSubmit, preventDefault = true, children, ...props }, ref) => {
    const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
      if (preventDefault) {
        e.preventDefault();
      }
      onSubmit?.(e);
    };

    return (
      <Box
        ref={ref}
        component="form"
        onSubmit={handleSubmit}
        noValidate
        autoComplete="off"
        {...(props as BoxProps<'form'>)}
      >
        {children}
      </Box>
    );
  },
);

FormContainer.displayName = 'FormContainer';

export default FormContainer;
