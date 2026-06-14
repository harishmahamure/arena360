'use client';

import { useCallback, useRef, useState } from 'react';

export interface UseAsyncActionReturn {
  /** True while an async action is in flight (drives UI disabled/loading). */
  loading: boolean;
  /** Alias for `loading`. */
  isPending: boolean;
  /**
   * Runs `fn` once; ignores concurrent calls until the first completes.
   * Returns `undefined` when skipped due to an in-flight action.
   */
  run: <T>(fn: () => Promise<T>) => Promise<T | undefined>;
  /**
   * Runs `predicate` synchronously; if true, delegates to `run(fn)`.
   * Use for validation-before-lock patterns (e.g. POS checkout).
   */
  runIf: <T>(predicate: () => boolean, fn: () => Promise<T>) => Promise<T | undefined>;
}

export function useAsyncAction(): UseAsyncActionReturn {
  const [loading, setLoading] = useState(false);
  const inFlightRef = useRef(false);

  const run = useCallback(async <T>(fn: () => Promise<T>): Promise<T | undefined> => {
    if (inFlightRef.current) return undefined;
    inFlightRef.current = true;
    setLoading(true);
    try {
      return await fn();
    } finally {
      inFlightRef.current = false;
      setLoading(false);
    }
  }, []);

  const runIf = useCallback(
    async <T>(predicate: () => boolean, fn: () => Promise<T>): Promise<T | undefined> => {
      if (!predicate()) return undefined;
      return run(fn);
    },
    [run],
  );

  return { loading, isPending: loading, run, runIf };
}
