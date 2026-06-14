'use client';

import { useCallback, useMemo, useRef, useState } from 'react';

export interface UseAsyncActionOptions {
  /** Minimum milliseconds between accepted `run()` calls. @default 0 */
  throttleMs?: number;
  /** When true, a successful `run()` locks the action until `reset()`. @default false */
  lockOnSuccess?: boolean;
}

export interface UseAsyncActionReturn {
  /** True while an async action is in flight (drives UI disabled/loading). */
  loading: boolean;
  /** Alias for `loading`. */
  isPending: boolean;
  /** True after a successful `run()` when `lockOnSuccess` is enabled. */
  succeeded: boolean;
  /** True after the last `run()` threw. Cleared on the next accepted `run()`. */
  failed: boolean;
  /** Message from the last thrown error, or null. */
  errorMessage: string | null;
  /** `loading || succeeded` — combine with form validation for button `disabled`. */
  disabled: boolean;
  /**
   * Runs `fn` once; ignores concurrent, throttled, or post-success calls.
   * Returns `undefined` when skipped. Rethrows errors after recording `failed` / `errorMessage`.
   */
  run: <T>(fn: () => Promise<T>) => Promise<T | undefined>;
  /**
   * Runs `predicate` synchronously; if true, delegates to `run(fn)`.
   * Use for validation-before-lock patterns (e.g. POS checkout).
   */
  runIf: <T>(predicate: () => boolean, fn: () => Promise<T>) => Promise<T | undefined>;
  /** Clears `failed` and `errorMessage` without resetting success lock. */
  clearError: () => void;
  /** Clears success, error, and in-flight guard (e.g. when reopening a dialog). */
  reset: () => void;
}

export function useAsyncAction(options?: UseAsyncActionOptions): UseAsyncActionReturn {
  const throttleMs = options?.throttleMs ?? 0;
  const lockOnSuccess = options?.lockOnSuccess ?? false;

  const [loading, setLoading] = useState(false);
  const [succeeded, setSucceeded] = useState(false);
  const [failed, setFailed] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const inFlightRef = useRef(false);
  const lastRunAtRef = useRef(0);
  const succeededRef = useRef(false);

  const clearError = useCallback(() => {
    setFailed(false);
    setErrorMessage(null);
  }, []);

  const reset = useCallback(() => {
    succeededRef.current = false;
    setSucceeded(false);
    setLoading(false);
    setFailed(false);
    setErrorMessage(null);
    inFlightRef.current = false;
  }, []);

  const run = useCallback(
    async <T>(fn: () => Promise<T>): Promise<T | undefined> => {
      if (inFlightRef.current || (lockOnSuccess && succeededRef.current)) return undefined;

      const now = Date.now();
      if (throttleMs > 0 && now - lastRunAtRef.current < throttleMs) return undefined;

      lastRunAtRef.current = now;
      inFlightRef.current = true;
      setLoading(true);
      setFailed(false);
      setErrorMessage(null);

      try {
        const result = await fn();
        if (lockOnSuccess) {
          succeededRef.current = true;
          setSucceeded(true);
        }
        return result;
      } catch (err) {
        setFailed(true);
        setErrorMessage(err instanceof Error ? err.message : 'Something went wrong');
        throw err;
      } finally {
        inFlightRef.current = false;
        setLoading(false);
      }
    },
    [throttleMs, lockOnSuccess],
  );

  const runIf = useCallback(
    async <T>(predicate: () => boolean, fn: () => Promise<T>): Promise<T | undefined> => {
      if (!predicate()) return undefined;
      return run(fn);
    },
    [run],
  );

  const disabled = useMemo(() => loading || succeeded, [loading, succeeded]);

  return {
    loading,
    isPending: loading,
    succeeded,
    failed,
    errorMessage,
    disabled,
    run,
    runIf,
    clearError,
    reset,
  };
}
