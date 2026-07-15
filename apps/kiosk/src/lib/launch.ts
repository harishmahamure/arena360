import {
  allowListPaths,
  type LaunchEntry,
  launchViaLabel,
  loadLaunchEntries,
  normalizeLaunchArguments,
  resolveLaunch,
} from './allowList';
import { launchAllowed, launchAllowedTest } from './tauriCommands';

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
  if (!entry.launchVia) return;
  if (!entry.launchVia.executablePath.trim()) {
    throw new Error(`Launcher path is required to launch ${entry.name}`);
  }
  const args = normalizeLaunchArguments(entry.launchVia.arguments);
  if (!args?.length) {
    throw new Error(`Launcher arguments are required to launch ${entry.name}`);
  }
}

function prepareLaunch(
  entry: LaunchEntry,
  entries: LaunchEntry[],
): { executablePath: string; allowList: string[]; arguments?: string[] } {
  validateLaunchEntry(entry);
  const allowList = allowListPaths(entries);
  if (allowList.length === 0) {
    throw new Error('No applications are configured on this station');
  }
  const resolved = resolveLaunch(entry);

  if (
    entry.launchVia &&
    !allowList.some((p) => p.toLowerCase() === entry.launchVia?.executablePath.toLowerCase())
  ) {
    throw new Error(
      `${launchViaLabel(entry) ?? 'Launcher'} must be on the allow-list before launching ${entry.name}`,
    );
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
  const prepared = prepareLaunch(entry, entries);
  await launchAllowed(
    prepared.executablePath,
    prepared.allowList,
    prepared.arguments,
    isSoftLaunchEntry(entry),
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
  const prepared = prepareLaunch(entry, entries);
  await launchAllowedTest(prepared.executablePath, prepared.allowList, prepared.arguments);
}
