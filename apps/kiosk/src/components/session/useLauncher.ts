import { useCallback, useMemo, useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import { installedEntries, launchEntry } from '../../lib/launch';

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

  const isLaunchable = useCallback((entry: LaunchEntry) => entry.present !== false, []);

  const launch = useCallback(
    async (entry: LaunchEntry) => {
      if (disabled || entry.present === false) return;
      setLaunchingKey(entry.id);
      try {
        await launchEntry(entry, entries);
        onLaunched?.();
      } catch (e) {
        onError?.(e instanceof Error ? e.message : `Could not launch ${entry.name}`);
      } finally {
        setLaunchingKey(null);
      }
    },
    [disabled, entries, onError, onLaunched],
  );

  return { entries, launchingKey, isLaunchable, launch };
}
