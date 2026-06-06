import { useMemo } from 'react';
import type { LaunchEntry } from '../../lib/allowList';

/** Load an allow-list slice (games or tools) for player views. */
export function useAllowList(loader: () => LaunchEntry[]): { items: LaunchEntry[] } {
  const items = useMemo(() => loader(), [loader]);
  return { items };
}
