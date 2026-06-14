'use client';

import { CheckCircle, ErrorOutline } from '@mui/icons-material';
import {
  Button,
  type ButtonProps,
  CircularProgress,
  Fade,
  keyframes,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { forwardRef, useEffect, useRef, useState } from 'react';

export interface FormButtonProps extends ButtonProps {
  /**
   * If true, shows a loading spinner and disables the button
   * @default false
   */
  loading?: boolean;
  /**
   * If true, shows a success checkmark and disables the button
   * @default false
   */
  success?: boolean;
  /**
   * Label shown when `success` is true (falls back to children)
   */
  successLabel?: string;
  /**
   * If true, shows an error icon and label; button stays enabled for retry
   * @default false
   */
  error?: boolean;
  /**
   * Label shown when `error` is true (falls back to children)
   */
  errorLabel?: string;
  /**
   * Custom loading indicator component
   */
  loadingIndicator?: React.ReactNode;
}

type ActionVisualState = 'idle' | 'loading' | 'success' | 'error';

const actionButtonShake = keyframes`
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-3px); }
  40% { transform: translateX(3px); }
  60% { transform: translateX(-2px); }
  80% { transform: translateX(2px); }
`;

const successIconPop = keyframes`
  from { transform: scale(0.9); opacity: 0.6; }
  to { transform: scale(1); opacity: 1; }
`;

function resolveVisualState(loading: boolean, success: boolean, error: boolean): ActionVisualState {
  if (success) return 'success';
  if (error) return 'error';
  if (loading) return 'loading';
  return 'idle';
}

/**
 * FormButton - A reusable button component with loading state support
 * Wraps Material-UI Button with loading functionality
 */
const FormButton = forwardRef<HTMLButtonElement, FormButtonProps>(
  (
    {
      loading = false,
      success = false,
      successLabel,
      error = false,
      errorLabel,
      loadingIndicator,
      children,
      disabled,
      startIcon,
      sx,
      color = 'primary',
      ...props
    },
    ref,
  ) => {
    const theme = useTheme();
    const prefersReducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)');
    const visualState = resolveVisualState(loading, success, error);
    const prevVisualStateRef = useRef<ActionVisualState>(visualState);
    const [shake, setShake] = useState(false);

    useEffect(() => {
      const prev = prevVisualStateRef.current;
      if (visualState === 'error' && prev !== 'error' && !prefersReducedMotion) {
        setShake(true);
        const timer = window.setTimeout(() => setShake(false), 300);
        prevVisualStateRef.current = visualState;
        return () => window.clearTimeout(timer);
      }
      prevVisualStateRef.current = visualState;
    }, [visualState, prefersReducedMotion]);

    const isDisabled = disabled || loading || success;

    const defaultLoadingIndicator = <CircularProgress size={20} color="inherit" />;

    const stateColor =
      visualState === 'success'
        ? theme.palette.success.main
        : visualState === 'error'
          ? theme.palette.error.main
          : undefined;

    const content =
      visualState === 'success' ? (
        <>
          <CheckCircle
            fontSize="small"
            sx={{
              animation: prefersReducedMotion ? undefined : `${successIconPop} 150ms ease-out`,
            }}
          />
          <span aria-live="polite">{successLabel ?? children}</span>
        </>
      ) : visualState === 'error' ? (
        <>
          <ErrorOutline fontSize="small" />
          <span aria-live="polite">{errorLabel ?? children}</span>
        </>
      ) : visualState === 'loading' ? (
        <>
          {loadingIndicator || defaultLoadingIndicator}
          <span>{children}</span>
        </>
      ) : (
        children
      );

    return (
      <Button
        ref={ref}
        disabled={isDisabled}
        color={visualState === 'idle' || visualState === 'loading' ? color : undefined}
        startIcon={visualState === 'idle' ? startIcon : undefined}
        sx={{
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 1,
          transition: prefersReducedMotion
            ? undefined
            : 'background-color 250ms ease, color 250ms ease, border-color 250ms ease',
          ...(stateColor
            ? {
                backgroundColor: stateColor,
                color: theme.palette.getContrastText(stateColor),
                '&:hover': {
                  backgroundColor: stateColor,
                },
              }
            : {}),
          ...(visualState === 'loading' && !stateColor ? { opacity: 0.85 } : {}),
          ...(shake && !prefersReducedMotion
            ? { animation: `${actionButtonShake} 300ms ease-in-out` }
            : {}),
          ...sx,
        }}
        {...props}
      >
        <Fade in key={visualState} timeout={prefersReducedMotion ? 0 : 200}>
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 8,
            }}
          >
            {content}
          </span>
        </Fade>
      </Button>
    );
  },
);

FormButton.displayName = 'FormButton';

export default FormButton;
