import { useCallback, useMemo, useRef, useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import { appendKioskLog } from '../../lib/bootDiagnostics';
import { installedEntries, launchEntry, launchErrorMessage } from '../../lib/launch';

interface UseLauncher {
  entries: LaunchEntry[];
  launchingKey: string | null;
  isLaunchable: (entry: LaunchEntry) => boolean;
  launch: (entry: LaunchEntry) => Promise<void>;
}

/** Shared launch state for in-session views (ADR-0019). */
export function useLauncher(
  disabled: boolean,
  onError?: (message: string) => void,
  onLaunched?: () => void,
): UseLauncher {
  const entries = useMemo(() => installedEntries(), []);
  const [launchingKey, setLaunchingKey] = useState<string | null>(null);
  const launchingRef = useRef(false);

  const isLaunchable = useCallback((entry: LaunchEntry) => entry.present !== false, []);

  const launch = useCallback(
    async (entry: LaunchEntry) => {
      if (disabled || entry.present === false || launchingRef.current) return;
      launchingRef.current = true;
      setLaunchingKey(entry.id);
      try {
        await launchEntry(entry, entries);
        onLaunched?.();
      } catch (e) {
        const message = launchErrorMessage(e, `Could not launch ${entry.name}`);
        void appendKioskLog(
          'error',
          `launch failed entry=${entry.id} name=${entry.name}: ${message}`,
        );
        onError?.(message);
      } finally {
        launchingRef.current = false;
        setLaunchingKey(null);
      }
    },
    [disabled, entries, onError, onLaunched],
  );

  return { entries, launchingKey, isLaunchable, launch };
}
