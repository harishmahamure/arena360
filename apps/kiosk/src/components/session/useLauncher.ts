import { useCallback, useMemo, useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import type { KioskGame } from '../../lib/games';
import { installedEntries, launchEntry, resolveEntry } from '../../lib/launch';

type Launchable = Pick<KioskGame, 'id' | 'name' | 'launchRef'>;

interface UseLauncher {
  /** Installed allow-list entries on this station. */
  entries: LaunchEntry[];
  /** Key of the entry currently launching, or null. */
  launchingKey: string | null;
  /** Resolve whether a game/hero maps to an installed entry. */
  isLaunchable: (item: Launchable) => boolean;
  /** Launch a game/hero through the native allow-list guard (ADR-0019). */
  launch: (item: Launchable) => Promise<void>;
}

/**
 * Shared launch state for the in-session views. Resolves catalog entries to the
 * client-side allow-list and tracks the in-flight launch for tile feedback.
 */
export function useLauncher(disabled: boolean, onError?: (message: string) => void): UseLauncher {
  const entries = useMemo(() => installedEntries(), []);
  const [launchingKey, setLaunchingKey] = useState<string | null>(null);

  const isLaunchable = useCallback(
    (item: Launchable) => Boolean(resolveEntry(item, entries)),
    [entries],
  );

  const launch = useCallback(
    async (item: Launchable) => {
      if (disabled) return;
      const entry = resolveEntry(item, entries);
      if (!entry) return;
      setLaunchingKey(item.id);
      try {
        await launchEntry(entry, entries);
      } catch (e) {
        onError?.(e instanceof Error ? e.message : `Could not launch ${item.name}`);
      } finally {
        setLaunchingKey(null);
      }
    },
    [disabled, entries, onError],
  );

  return { entries, launchingKey, isLaunchable, launch };
}
