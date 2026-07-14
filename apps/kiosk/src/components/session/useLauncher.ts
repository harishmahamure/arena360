import { useCallback, useRef, useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import { loadLaunchEntries } from '../../lib/allowList';
import { appendKioskLog } from '../../lib/bootDiagnostics';
import { installedEntries, launchEntry, launchErrorMessage } from '../../lib/launch';

interface UseLauncher {
  entries: LaunchEntry[];
  launchingKey: string | null;
  isLaunchable: (entry: LaunchEntry) => boolean;
  launch: (entry: LaunchEntry) => Promise<void>;
  refreshEntries: () => void;
}

/** Shared launch state for in-session views (ADR-0019). */
export function useLauncher(
  disabled: boolean,
  onError?: (message: string) => void,
  onLaunched?: () => void,
): UseLauncher {
  const [entries, setEntries] = useState<LaunchEntry[]>(() => installedEntries());
  const [launchingKey, setLaunchingKey] = useState<string | null>(null);
  const launchingRef = useRef(false);

  const refreshEntries = useCallback(() => {
    setEntries(installedEntries());
  }, []);

  const isLaunchable = useCallback((entry: LaunchEntry) => entry.present !== false, []);

  const launch = useCallback(
    async (entry: LaunchEntry) => {
      if (disabled || entry.present === false || launchingRef.current) return;
      launchingRef.current = true;
      setLaunchingKey(entry.id);
      const currentEntries = loadLaunchEntries().filter((e) => e.present !== false);
      setEntries(currentEntries);
      try {
        await launchEntry(entry, currentEntries);
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
    [disabled, onError, onLaunched],
  );

  return { entries, launchingKey, isLaunchable, launch, refreshEntries };
}
