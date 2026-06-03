import type { LaunchCategory } from './allowList';

/**
 * Kiosk games catalog (ADR-0019). The catalog seeds from
 * `public/games/games.json` and, once an admin edits it in Setup mode, persists
 * to localStorage (same client-only model as the launch allow-list). It decides
 * what is *shown*; launching still resolves through the allow-list
 * (`lib/launch.ts`), so an entry only launches when its `launchRef` matches an
 * installed executable.
 */

export interface KioskGame {
  id: string;
  name: string;
  genre?: string | null;
  description?: string | null;
  thumbnailUrl?: string | null;
  logoUrl?: string | null;
  videoUrl?: string | null;
  category?: LaunchCategory;
  icon?: string | null;
  subtitle?: string | null;
  launchRef?: string | null;
  isActive?: boolean;
  sortOrder?: number;
}

interface RawGame {
  id?: unknown;
  name?: unknown;
  genre?: unknown;
  description?: unknown;
  thumbnailUrl?: unknown;
  logoUrl?: unknown;
  videoUrl?: unknown;
  category?: unknown;
  icon?: unknown;
  subtitle?: unknown;
  launchRef?: unknown;
  isActive?: unknown;
  sortOrder?: unknown;
}

interface GamesFile {
  games?: RawGame[];
}

const STORAGE_KEY = 'gaming-cafe.kiosk.catalog';
const SEED_URL = '/games/games.json';
const CATEGORIES: readonly LaunchCategory[] = ['game', 'launcher', 'util'];

function str(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function category(value: unknown): LaunchCategory {
  return CATEGORIES.includes(value as LaunchCategory) ? (value as LaunchCategory) : 'game';
}

/** Normalise an arbitrary record (seed file or stored value) into a KioskGame. */
function toGame(raw: RawGame): KioskGame | null {
  const id = str(raw.id);
  const name = str(raw.name);
  if (!id || !name) return null;
  return {
    id,
    name,
    genre: str(raw.genre),
    description: str(raw.description),
    thumbnailUrl: str(raw.thumbnailUrl),
    logoUrl: str(raw.logoUrl),
    videoUrl: str(raw.videoUrl),
    category: category(raw.category),
    icon: str(raw.icon),
    subtitle: str(raw.subtitle),
    launchRef: str(raw.launchRef),
    isActive: raw.isActive !== false,
    sortOrder: typeof raw.sortOrder === 'number' ? raw.sortOrder : 0,
  };
}

function bySort(a: KioskGame, b: KioskGame): number {
  return (a.sortOrder ?? 0) - (b.sortOrder ?? 0);
}

/** In-memory copy of the catalog (seed or stored), populated on first load. */
let catalog: KioskGame[] | null = null;

function readStore(): KioskGame[] | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const value = JSON.parse(raw);
    if (!Array.isArray(value)) return null;
    return value.map(toGame).filter((g): g is KioskGame => g !== null);
  } catch {
    return null;
  }
}

async function fetchSeed(): Promise<KioskGame[]> {
  try {
    const res = await fetch(SEED_URL, { cache: 'no-cache' });
    if (!res.ok) return [];
    const file = (await res.json()) as GamesFile;
    return (file.games ?? []).map(toGame).filter((g): g is KioskGame => g !== null);
  } catch {
    return [];
  }
}

/**
 * Load the catalog once: prefer the admin-edited localStorage copy, otherwise
 * fall back to the shipped seed (without persisting, so seed updates apply until
 * the first edit).
 */
async function ensureLoaded(): Promise<KioskGame[]> {
  if (catalog) return catalog;
  const stored = readStore();
  catalog = stored ?? (await fetchSeed());
  return catalog;
}

function persist(list: KioskGame[]): KioskGame[] {
  catalog = list;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    // localStorage unavailable — non-fatal
  }
  return list;
}

function newId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
  return `game_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

/** Full catalog sorted by `sortOrder` (for the admin editor and views). */
export async function fetchCatalog(): Promise<KioskGame[]> {
  return [...(await ensureLoaded())].sort(bySort);
}

/** Active catalog entries shown on Home / Library. */
export async function fetchActiveGames(): Promise<KioskGame[]> {
  return (await fetchCatalog()).filter(
    (g) => g.isActive !== false && (g.category ?? 'game') === 'game',
  );
}

/** Active launcher/utility entries shown under Settings & Tools. */
export async function fetchTools(): Promise<KioskGame[]> {
  return (await fetchCatalog()).filter(
    (g) => g.isActive !== false && (g.category === 'launcher' || g.category === 'util'),
  );
}

/** Persist the full catalog (admin editor save). */
export function saveCatalog(list: KioskGame[]): KioskGame[] {
  return persist([...list]);
}

/** Create or update a single catalog game, returning the new full list. */
export function upsertGame(game: KioskGame): KioskGame[] {
  const list = catalog ?? readStore() ?? [];
  const id = game.id || newId();
  const next = { ...game, id };
  const idx = list.findIndex((g) => g.id === id);
  const updated = idx >= 0 ? list.map((g) => (g.id === id ? next : g)) : [...list, next];
  return persist(updated);
}

/** Remove a catalog game by id, returning the new full list. */
export function removeGame(id: string): KioskGame[] {
  const list = catalog ?? readStore() ?? [];
  return persist(list.filter((g) => g.id !== id));
}

/** A blank game template for the add form. */
export function blankGame(): KioskGame {
  return {
    id: '',
    name: '',
    category: 'game',
    isActive: true,
    sortOrder: 0,
  };
}
