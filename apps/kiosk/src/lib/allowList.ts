/**
 * Client-side launch allow-list (ADR-0019). The curated list of executables a
 * player may launch lives only in the kiosk's localStorage — there is no
 * backend persistence in v1. The setup-mode editor reads/writes this list and
 * the player launcher grid renders from it.
 */

const STORAGE_KEY = 'gaming-cafe.kiosk.launch_entries';

export interface LaunchEntry {
  /** Stable id (generated on add). */
  id: string;
  /** Display label shown on the launcher tile. */
  name: string;
  /** Absolute executable path passed to `launch_allowed`. */
  executablePath: string;
  /** Optional launch arguments. */
  arguments?: string;
  /** Whether the executable was present at last scan (UI hint only). */
  present?: boolean;
}

function safeParse(raw: string | null): LaunchEntry[] {
  if (!raw) return [];
  try {
    const value = JSON.parse(raw);
    if (!Array.isArray(value)) return [];
    return value.filter(
      (e): e is LaunchEntry =>
        typeof e?.id === 'string' &&
        typeof e?.name === 'string' &&
        typeof e?.executablePath === 'string',
    );
  } catch {
    return [];
  }
}

export function loadLaunchEntries(): LaunchEntry[] {
  try {
    return safeParse(localStorage.getItem(STORAGE_KEY));
  } catch {
    return [];
  }
}

export function saveLaunchEntries(entries: LaunchEntry[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // localStorage unavailable — non-fatal
  }
}

function newId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  return `entry_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

export function addLaunchEntry(entry: Omit<LaunchEntry, 'id'>): LaunchEntry[] {
  const entries = loadLaunchEntries();
  const exists = entries.some(
    (e) => e.executablePath.toLowerCase() === entry.executablePath.toLowerCase(),
  );
  if (exists) return entries;
  const next = [...entries, { ...entry, id: newId() }];
  saveLaunchEntries(next);
  return next;
}

export function removeLaunchEntry(id: string): LaunchEntry[] {
  const next = loadLaunchEntries().filter((e) => e.id !== id);
  saveLaunchEntries(next);
  return next;
}

/** Paths passed to `launch_allowed` as the allow-list snapshot. */
export function allowListPaths(entries: LaunchEntry[] = loadLaunchEntries()): string[] {
  return entries.map((e) => e.executablePath);
}
