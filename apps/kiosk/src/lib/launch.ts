import { allowListPaths, type LaunchEntry, loadLaunchEntries } from './allowList';
import type { KioskGame } from './games';
import { launchAllowed } from './tauriCommands';

/**
 * Shared launch resolution for the Arena360 in-session views (Home / Library /
 * Settings). A catalog entry maps to a client-side allow-list entry by explicit
 * `launchRef` (entry id or name) or by matching display names (ADR-0019).
 */

/** Installed allow-list entries (present on this station). */
export function installedEntries(): LaunchEntry[] {
  return loadLaunchEntries().filter((e) => e.present !== false);
}

/** Resolve the allow-list entry a catalog game launches, if any. */
export function resolveEntry(
  game: Pick<KioskGame, 'name' | 'launchRef'>,
  entries: LaunchEntry[],
): LaunchEntry | undefined {
  const ref = game.launchRef?.toLowerCase();
  if (ref) {
    const byId = entries.find((e) => e.id.toLowerCase() === ref);
    if (byId) return byId;
    const byName = entries.find((e) => e.name.toLowerCase() === ref);
    if (byName) return byName;
  }
  return entries.find((e) => e.name.toLowerCase() === game.name.toLowerCase());
}

/** Launch an allow-list entry through the native guard (ADR-0019/0020). */
export async function launchEntry(entry: LaunchEntry, entries: LaunchEntry[]): Promise<void> {
  await launchAllowed(entry.executablePath, allowListPaths(entries), entry.arguments);
}
