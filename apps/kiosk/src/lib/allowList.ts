/**
 * Client-side launch allow-list (ADR-0019). The curated list of executables a
 * player may launch lives only in the kiosk's localStorage. Setup mode edits
 * this list; player Home / Library / Tools render directly from it. Optional
 * media URLs are picked from the centrally hosted CDN gallery.
 */

import type { ScanCandidate } from './tauriCommands';

const STORAGE_KEY = 'gaming-cafe.kiosk.launch_entries';

/** Scan sources auto-imported on merge (ADR-0019 merge-new-only). */
export const TRUSTED_SCAN_SOURCES = ['known', 'steam', 'manifest'] as const;

/** Player-home section an entry is grouped under. */
export type LaunchCategory = 'game' | 'launcher' | 'util';

/** Launch through a platform launcher (Riot Client, Steam, etc.). */
export interface LaunchVia {
  executablePath: string;
  arguments: string | string[];
}

/** Tokenize a command-line string respecting double quotes (Windows-style). */
export function tokenizeArguments(input: string): string[] {
  const out: string[] = [];
  let current = '';
  let inQuotes = false;
  for (const ch of input) {
    if (ch === '"') {
      inQuotes = !inQuotes;
      continue;
    }
    if ((ch === ' ' || ch === '\t') && !inQuotes) {
      if (current) {
        out.push(current);
        current = '';
      }
      continue;
    }
    current += ch;
  }
  if (current) out.push(current);
  return out;
}

/** Normalize launch arguments to a argv list for native spawn. */
export function normalizeLaunchArguments(args?: string | string[]): string[] | undefined {
  if (args == null) return undefined;
  const list = Array.isArray(args) ? args : tokenizeArguments(args);
  const trimmed = list.map((a) => a.trim()).filter(Boolean);
  return trimmed.length > 0 ? trimmed : undefined;
}

/** Serialize launch arguments for storage / display in a single text field. */
export function formatLaunchArguments(args: string | string[]): string {
  return Array.isArray(args) ? args.join(' ') : args;
}

export interface LaunchEntry {
  /** Stable id (generated on add). */
  id: string;
  /** Display label shown on the launcher tile. */
  name: string;
  /** Absolute executable path passed to `launch_allowed`. */
  executablePath: string;
  /** Optional launch arguments (direct launch only). */
  arguments?: string;
  /** When set, launch goes through the launcher executable + args. */
  launchVia?: LaunchVia;
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
  const paths = new Set<string>();
  for (const entry of entries) {
    paths.add(entry.executablePath);
    if (entry.launchVia?.executablePath) {
      paths.add(entry.launchVia.executablePath);
    }
  }
  return [...paths];
}

export interface ResolvedLaunch {
  executablePath: string;
  arguments?: string[];
  /** Display / presence path from the allow-list entry. */
  entryPath: string;
}

/** Resolve direct vs launcher-mediated launch for an entry. */
export function resolveLaunch(entry: LaunchEntry): ResolvedLaunch {
  if (entry.launchVia) {
    return {
      executablePath: entry.launchVia.executablePath,
      arguments: normalizeLaunchArguments(entry.launchVia.arguments),
      entryPath: entry.executablePath,
    };
  }
  return {
    executablePath: entry.executablePath,
    arguments: normalizeLaunchArguments(entry.arguments),
    entryPath: entry.executablePath,
  };
}

function executableBaseName(path: string): string {
  return (
    path
      .split(/[/\\]/)
      .pop()
      ?.replace(/\.exe$/i, '') ?? 'Launcher'
  );
}

/** Human label for a launcher executable path (e.g. "Riot Client"). */
export function launcherDisplayName(executablePath: string): string {
  const base = executableBaseName(executablePath);
  if (/riotclient/i.test(base)) return 'Riot Client';
  if (/steam/i.test(base)) return 'Steam';
  if (/epic/i.test(base)) return 'Epic Games';
  return base.replace(/[_-]+/g, ' ');
}

/** Human label for a launcher-mediated entry (e.g. "Riot Client"). */
export function launchViaLabel(entry: LaunchEntry): string | null {
  if (!entry.launchVia) return null;
  return launcherDisplayName(entry.launchVia.executablePath);
}

function pathsEqual(a: string, b: string): boolean {
  return a.toLowerCase() === b.toLowerCase();
}

function findByPath(entries: LaunchEntry[], path: string): LaunchEntry | undefined {
  return entries.find((e) => pathsEqual(e.executablePath, path));
}

/** Map a scan candidate to a new allow-list entry (no id). */
export function candidateToEntry(candidate: ScanCandidate): Omit<LaunchEntry, 'id'> {
  return {
    name: candidate.name,
    executablePath: candidate.executablePath,
    present: candidate.present,
    category: categorizeByName(candidate.name),
    launchVia: candidate.launchVia
      ? {
          executablePath: candidate.launchVia.executablePath,
          arguments: candidate.launchVia.arguments,
        }
      : undefined,
  };
}

export interface MergeScanResult {
  entries: LaunchEntry[];
  added: number;
  updated: number;
  launchersAdded: number;
}

/** True when scan results from this source are auto-imported after scan. */
export function isTrustedScanSource(source: string): boolean {
  return (TRUSTED_SCAN_SOURCES as readonly string[]).includes(source);
}

/**
 * Merge trusted, present scan candidates into the allow-list (ADR-0019 merge-new-only).
 * Auto-adds launcher executables required by profiled games.
 */
export function mergeScanCandidates(
  candidates: ScanCandidate[],
  options?: { sources?: readonly string[] },
): MergeScanResult {
  const sources = new Set(options?.sources ?? TRUSTED_SCAN_SOURCES);
  let entries = loadLaunchEntries();
  let added = 0;
  let updated = 0;
  let launchersAdded = 0;

  const trusted = candidates.filter((c) => c.present && sources.has(c.source));

  for (const candidate of trusted) {
    const existing = findByPath(entries, candidate.executablePath);
    if (!existing) {
      const draft = candidateToEntry(candidate);
      entries.push({
        ...draft,
        category: draft.category ?? categorizeByName(draft.name),
        id: newId(),
      });
      added += 1;
    } else {
      const patch: Partial<LaunchEntry> = {};
      if (existing.present !== true) patch.present = true;
      if (candidate.launchVia) {
        const nextVia = {
          executablePath: candidate.launchVia.executablePath,
          arguments: candidate.launchVia.arguments,
        };
        const existingVia = existing.launchVia;
        const viaChanged =
          !existingVia ||
          existingVia.executablePath !== nextVia.executablePath ||
          JSON.stringify(normalizeLaunchArguments(existingVia.arguments)) !==
            JSON.stringify(normalizeLaunchArguments(nextVia.arguments));
        if (viaChanged) {
          patch.launchVia = nextVia;
        }
      }
      if (Object.keys(patch).length > 0) {
        entries = entries.map((e) => (e.id === existing.id ? { ...e, ...patch, id: e.id } : e));
        updated += 1;
      }
    }

    if (candidate.launchVia) {
      const launcherPath = candidate.launchVia.executablePath;
      if (!findByPath(entries, launcherPath)) {
        entries.push({
          id: newId(),
          name: launcherDisplayName(launcherPath),
          executablePath: launcherPath,
          present: true,
          category: 'launcher',
        });
        added += 1;
        launchersAdded += 1;
      }
    }
  }

  saveLaunchEntries(entries);
  return { entries, added, updated, launchersAdded };
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
