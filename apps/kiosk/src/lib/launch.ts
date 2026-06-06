import { allowListPaths, type LaunchEntry, loadLaunchEntries } from './allowList';
import { launchAllowed } from './tauriCommands';

/** Allow-list entries available to launch (executable present at last scan). */
export function installedEntries(): LaunchEntry[] {
  return loadLaunchEntries().filter((e) => e.present !== false);
}

/** Launch an allow-list entry through the native guard (ADR-0019/0020). */
export async function launchEntry(entry: LaunchEntry, entries: LaunchEntry[]): Promise<void> {
  await launchAllowed(entry.executablePath, allowListPaths(entries), entry.arguments);
}
