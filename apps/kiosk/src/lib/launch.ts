import {
  allowListPaths,
  entryLaunchMode,
  type LaunchEntry,
  loadLaunchEntries,
  resolveLaunch,
} from './allowList';
import { detectLaunchProfile, launchAllowed, launchAllowedTest } from './tauriCommands';

/** Extract a user-visible message from a launch failure (Tauri rejects with strings). */
export function launchErrorMessage(e: unknown, fallback: string): string {
  const msg = e instanceof Error ? e.message : String(e);
  return msg.trim() || fallback;
}

/** Allow-list entries available to launch (executable present at last scan). */
export function installedEntries(): LaunchEntry[] {
  return loadLaunchEntries().filter((e) => e.present !== false);
}

function validateLaunchEntry(entry: LaunchEntry): void {
  if (entry.launchMode === 'direct') return;
  if (
    (entry.launchMode === 'launcher' || entry.launchMode === 'launcher-only') &&
    !entry.launchVia
  ) {
    throw new Error(`Launcher path is required to launch ${entry.name}`);
  }
  if (!entry.launchVia) return;
  if (!entry.launchVia.executablePath.trim()) {
    throw new Error(`Launcher path is required to launch ${entry.name}`);
  }
}

async function prepareLaunch(
  entry: LaunchEntry,
  entries: LaunchEntry[],
): Promise<{ executablePath: string; allowList: string[]; arguments?: string[] }> {
  if (entries.length === 0) {
    throw new Error('No applications are configured on this station');
  }
  if (!entries.some((e) => e.executablePath.toLowerCase() === entry.executablePath.toLowerCase())) {
    throw new Error('Executable not in allow-list');
  }
  let detectedLauncher = false;
  // Resolve legacy/manual game paths at launch time too, without requiring a re-scan.
  if (entryLaunchMode(entry) === 'auto' && !entry.launchVia && !isSoftLaunchEntry(entry)) {
    const launchVia = await detectLaunchProfile(entry.executablePath);
    if (launchVia) {
      entry = { ...entry, launchVia, launchViaAuto: true };
      detectedLauncher = true;
    }
  }
  validateLaunchEntry(entry);
  const allowList = allowListPaths(entries);
  if (allowList.length === 0) {
    throw new Error('No applications are configured on this station');
  }
  const resolved = resolveLaunch(entry);

  if (detectedLauncher && entry.launchVia && !allowList.includes(entry.launchVia.executablePath)) {
    allowList.push(entry.launchVia.executablePath);
  }
  if (!allowList.some((path) => path.toLowerCase() === resolved.executablePath.toLowerCase())) {
    throw new Error('Executable not in allow-list');
  }

  return {
    executablePath: resolved.executablePath,
    allowList,
    arguments: resolved.arguments,
  };
}

/** Soft-launch utilities/launchers (visible spawn, no game boost). */
export function isSoftLaunchEntry(entry: LaunchEntry): boolean {
  return entry.category === 'util' || entry.category === 'launcher';
}

/** Launch an allow-list entry through the native guard (ADR-0019/0020). */
export async function launchEntry(entry: LaunchEntry, entries: LaunchEntry[]): Promise<void> {
  const prepared = await prepareLaunch(entry, entries);
  await launchAllowed(
    prepared.executablePath,
    prepared.allowList,
    prepared.arguments,
    isSoftLaunchEntry(entry),
    entry.runAsAdmin,
  );
}

/** Setup Test launch — SetupRelaxed only; does not require a player session. */
export async function launchEntryForTest(
  entry: LaunchEntry,
  entries: LaunchEntry[],
): Promise<void> {
  if (entry.present === false) {
    throw new Error(`Executable not found for ${entry.name}`);
  }
  const prepared = await prepareLaunch(entry, entries);
  await launchAllowedTest(
    prepared.executablePath,
    prepared.allowList,
    prepared.arguments,
    entry.runAsAdmin,
  );
}
