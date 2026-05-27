'use client';

import { Button, type ButtonProps, CircularProgress } from '@mui/material';
import { forwardRef } from 'react';

export interface FormButtonProps extends ButtonProps {
  /**
   * If true, shows a loading spinner and disables the button
   * @default false
   */
  loading?: boolean;
  /**
   * Custom loading indicator component
   */
  loadingIndicator?: React.ReactNode;
}

/**
 * FormButton - A reusable button component with loading state support
 * Wraps Material-UI Button with loading functionality
 */
const FormButton = forwardRef<HTMLButtonElement, FormButtonProps>(
  ({ loading = false, loadingIndicator, children, disabled, startIcon, ...props }, ref) => {
    const isDisabled = disabled || loading;

    const defaultLoadingIndicator = <CircularProgress size={20} color="inherit" />;

    return (
      <Button ref={ref} disabled={isDisabled} startIcon={loading ? null : startIcon} {...props}>
        {loading ? (
          <>
            {loadingIndicator || defaultLoadingIndicator}
            <span style={{ marginLeft: 8 }}>{children}</span>
          </>
        ) : (
          children
        )}
      </Button>
    );
  },
);

FormButton.displayName = 'FormButton';

export default FormButton;
