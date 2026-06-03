/**
 * Media gallery for the games catalog. Admins curate a list of logos, posters
 * and preview videos here, then pick from it when adding/editing a game. Like
 * the catalog and allow-list, it seeds from a shipped file
 * (`public/games/gallery.json`) and persists admin edits to localStorage.
 */

export type GalleryKind = 'image' | 'video';

export interface GalleryItem {
  /** Stable id (generated on add). */
  id: string;
  /** Whether the asset is a still image (logo/poster) or a preview video. */
  kind: GalleryKind;
  /** Label shown in the picker. */
  name: string;
  /** Served path (e.g. `/games/images/x.svg`) or a remote URL. */
  url: string;
}

interface RawItem {
  id?: unknown;
  kind?: unknown;
  name?: unknown;
  url?: unknown;
}

interface GalleryFile {
  items?: RawItem[];
}

const STORAGE_KEY = 'gaming-cafe.kiosk.gallery';
const SEED_URL = '/games/gallery.json';

function toItem(raw: RawItem): GalleryItem | null {
  const url = typeof raw.url === 'string' && raw.url.length > 0 ? raw.url : null;
  if (!url) return null;
  const kind: GalleryKind = raw.kind === 'video' ? 'video' : 'image';
  const id = typeof raw.id === 'string' && raw.id.length > 0 ? raw.id : url;
  const name =
    typeof raw.name === 'string' && raw.name.length > 0 ? raw.name : url.split('/').pop() || url;
  return { id, kind, name, url };
}

let gallery: GalleryItem[] | null = null;

function readStore(): GalleryItem[] | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const value = JSON.parse(raw);
    if (!Array.isArray(value)) return null;
    return value.map(toItem).filter((i): i is GalleryItem => i !== null);
  } catch {
    return null;
  }
}

async function fetchSeed(): Promise<GalleryItem[]> {
  try {
    const res = await fetch(SEED_URL, { cache: 'no-cache' });
    if (!res.ok) return [];
    const file = (await res.json()) as GalleryFile;
    return (file.items ?? []).map(toItem).filter((i): i is GalleryItem => i !== null);
  } catch {
    return [];
  }
}

function persist(list: GalleryItem[]): GalleryItem[] {
  gallery = list;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    // localStorage unavailable — non-fatal
  }
  return list;
}

function newId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
  return `asset_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

/** Load the gallery once (stored copy if present, otherwise the shipped seed). */
export async function loadGallery(): Promise<GalleryItem[]> {
  if (gallery) return gallery;
  const stored = readStore();
  gallery = stored ?? (await fetchSeed());
  return gallery;
}

/** Add a gallery item, returning the new full list (ignores blank/duplicate urls). */
export function addGalleryItem(item: Omit<GalleryItem, 'id'>): GalleryItem[] {
  const list = gallery ?? readStore() ?? [];
  const url = item.url.trim();
  if (!url || list.some((i) => i.url === url)) return persist(list);
  const name = item.name.trim() || url.split('/').pop() || url;
  return persist([...list, { ...item, url, name, id: newId() }]);
}

/** Remove a gallery item by id, returning the new full list. */
export function removeGalleryItem(id: string): GalleryItem[] {
  const list = gallery ?? readStore() ?? [];
  return persist(list.filter((i) => i.id !== id));
}
