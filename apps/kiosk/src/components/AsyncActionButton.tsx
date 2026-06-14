import { type ButtonHTMLAttributes, type ReactNode, useEffect, useRef } from 'react';

export interface AsyncActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  loading?: boolean;
  success?: boolean;
  successLabel?: ReactNode;
  error?: boolean;
  errorLabel?: ReactNode;
  loadingLabel?: ReactNode;
}

type VisualState = 'idle' | 'loading' | 'success' | 'error';

function resolveVisualState(loading: boolean, success: boolean, error: boolean): VisualState {
  if (success) return 'success';
  if (error) return 'error';
  if (loading) return 'loading';
  return 'idle';
}

export function AsyncActionButton({
  loading = false,
  success = false,
  error = false,
  successLabel,
  errorLabel,
  loadingLabel,
  children,
  disabled,
  className,
  ...props
}: AsyncActionButtonProps) {
  const visualState = resolveVisualState(loading, success, error);
  const isDisabled = disabled || loading || success;
  const prevVisualStateRef = useRef<VisualState>(visualState);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const prev = prevVisualStateRef.current;
    if (visualState === 'error' && prev !== 'error' && buttonRef.current) {
      const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      if (!prefersReducedMotion) {
        buttonRef.current.classList.add('async-action-shake');
        const timer = window.setTimeout(() => {
          buttonRef.current?.classList.remove('async-action-shake');
        }, 300);
        prevVisualStateRef.current = visualState;
        return () => window.clearTimeout(timer);
      }
    }
    prevVisualStateRef.current = visualState;
  }, [visualState]);

  let content: ReactNode = children;
  if (visualState === 'success') {
    content = (
      <>
        <span className="async-action-check" aria-hidden>
          ✓
        </span>
        <span aria-live="polite">{successLabel ?? children}</span>
      </>
    );
  } else if (visualState === 'error') {
    content = (
      <>
        <span className="async-action-error-icon" aria-hidden>
          !
        </span>
        <span aria-live="polite">{errorLabel ?? children}</span>
      </>
    );
  } else if (visualState === 'loading') {
    content = <span>{loadingLabel ?? children}</span>;
  }

  return (
    <button
      type="button"
      ref={buttonRef}
      {...props}
      disabled={isDisabled}
      className={[
        className,
        visualState === 'success' ? 'async-action-success' : '',
        visualState === 'error' ? 'async-action-error' : '',
        visualState === 'loading' ? 'async-action-loading' : '',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {content}
    </button>
  );
}
