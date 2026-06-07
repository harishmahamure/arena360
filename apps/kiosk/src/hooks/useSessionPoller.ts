import { useEffect, useRef } from 'react';

/**
 * Polls the server for the authoritative remaining time. Normal cadence is
 * every 15s; inside the final minute it bursts to every 5s so auto-end and
 * last-second recharges resync quickly (D12/D13).
 */
export function useSessionPoller(
  remainingMinutes: number | undefined,
  sync: () => Promise<void>,
  active: boolean,
  intervalOverrideMs?: number,
): void {
  const syncRef = useRef(sync);
  syncRef.current = sync;

  const finalBurst = (remainingMinutes ?? Number.POSITIVE_INFINITY) <= 1;

  useEffect(() => {
    if (!active) return;
    const intervalMs = intervalOverrideMs ?? (finalBurst ? 5000 : 15000);
    const id = setInterval(() => {
      void syncRef.current();
    }, intervalMs);
    return () => clearInterval(id);
  }, [active, finalBurst, intervalOverrideMs]);
}
