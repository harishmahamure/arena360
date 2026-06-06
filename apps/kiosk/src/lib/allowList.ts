/**
 * Client-side launch allow-list (ADR-0019). The curated list of executables a
 * player may launch lives only in the kiosk's localStorage. Setup mode edits
 * this list; player Home / Library / Tools render directly from it. Optional
 * media URLs are picked from the centrally hosted CDN gallery.
 */

const STORAGE_KEY = 'gaming-cafe.kiosk.launch_entries';

/** Player-home section an entry is grouped under. */
export type LaunchCategory = 'game' | 'launcher' | 'util';

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
  /** Player-home section. Missing is treated as `game` for back-compat. */
  category?: LaunchCategory;
  /** Optional display media (from CDN gallery picker). */
  thumbnailUrl?: string | null;
  logoUrl?: string | null;
  videoUrl?: string | null;
  genre?: string | null;
  description?: string | null;
  sortOrder?: number;
  /** Material Symbol name for launcher/util tiles. */
  icon?: string | null;
  /** Subtitle under tool tiles in Settings. */
  subtitle?: string | null;
}

const LAUNCHER_HINTS = [
  'steam',
  'epic',
  'riot',
  'battle.net',
  'battlenet',
  'ea app',
  'ea desktop',
  'origin',
  'ubisoft',
  'uplay',
  'gog',
  'galaxy',
  'rockstar',
  'launcher',
];

const UTIL_HINTS = [
  'chrome',
  'edge',
  'firefox',
  'opera',
  'brave',
  'browser',
  'internet',
  'g hub',
  'ghub',
  'lghub',
  'logitech',
  'nvidia',
  'geforce',
  'control panel',
  'discord',
  'razer',
  'corsair',
  'steelseries',
  'obs',
  'spotify',
];

/** Best-effort default section from a display name (admin can override). */
export function categorizeByName(name: string): LaunchCategory {
  const n = name.toLowerCase();
  if (UTIL_HINTS.some((h) => n.includes(h))) return 'util';
  if (LAUNCHER_HINTS.some((h) => n.includes(h))) return 'launcher';
  return 'game';
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

function bySort(a: LaunchEntry, b: LaunchEntry): number {
  const order = (a.sortOrder ?? 0) - (b.sortOrder ?? 0);
  return order !== 0 ? order : a.name.localeCompare(b.name);
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
  const category = entry.category ?? categorizeByName(entry.name);
  const next = [...entries, { ...entry, category, id: newId() }];
  saveLaunchEntries(next);
  return next;
}

export function removeLaunchEntry(id: string): LaunchEntry[] {
  const next = loadLaunchEntries().filter((e) => e.id !== id);
  saveLaunchEntries(next);
  return next;
}

/** Update an entry's player-home section. */
export function setEntryCategory(id: string, category: LaunchCategory): LaunchEntry[] {
  const next = loadLaunchEntries().map((e) => (e.id === id ? { ...e, category } : e));
  saveLaunchEntries(next);
  return next;
}

/** Patch fields on an existing entry (media, metadata, category). */
export function updateLaunchEntry(id: string, patch: Partial<LaunchEntry>): LaunchEntry[] {
  const next = loadLaunchEntries().map((e) => (e.id === id ? { ...e, ...patch, id: e.id } : e));
  saveLaunchEntries(next);
  return next;
}

/** Resolve an entry's section, treating a missing value as `game`. */
export function entryCategory(entry: LaunchEntry): LaunchCategory {
  return entry.category ?? 'game';
}

/** Paths passed to `launch_allowed` as the allow-list snapshot. */
export function allowListPaths(entries: LaunchEntry[] = loadLaunchEntries()): string[] {
  return entries.map((e) => e.executablePath);
}

/** Game entries for Home / Library, sorted by sortOrder then name. */
export function fetchGames(): LaunchEntry[] {
  return loadLaunchEntries()
    .filter((e) => entryCategory(e) === 'game')
    .sort(bySort);
}

/** Launcher and utility entries for Settings & Tools. */
export function fetchTools(): LaunchEntry[] {
  return loadLaunchEntries()
    .filter((e) => {
      const cat = entryCategory(e);
      return cat === 'launcher' || cat === 'util';
    })
    .sort(bySort);
}
