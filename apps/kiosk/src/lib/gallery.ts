import { GALLERY_FALLBACK_URL, GALLERY_URL } from './config';
import { readGalleryCacheFs, writeGalleryCacheFs } from './tauriCommands';

/**
 * Read-only media gallery sourced from a centrally hosted CDN JSON file.
 * Kiosks fetch on boot and when the Setup picker opens; results are cached
 * in app-data for offline use. Bundled `public/games/gallery.json` is the
 * last-resort fallback.
 */

export type GalleryKind = 'image' | 'video';

export interface GalleryItem {
  id: string;
  kind: GalleryKind;
  name: string;
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

function toItem(raw: RawItem): GalleryItem | null {
  const url = typeof raw.url === 'string' && raw.url.length > 0 ? raw.url : null;
  if (!url) return null;
  const kind: GalleryKind = raw.kind === 'video' ? 'video' : 'image';
  const id = typeof raw.id === 'string' && raw.id.length > 0 ? raw.id : url;
  const name =
    typeof raw.name === 'string' && raw.name.length > 0 ? raw.name : url.split('/').pop() || url;
  return { id, kind, name, url };
}

function parseFile(file: GalleryFile): GalleryItem[] {
  return (file.items ?? []).map(toItem).filter((i): i is GalleryItem => i !== null);
}

let gallery: GalleryItem[] | null = null;

async function fetchUrl(url: string): Promise<GalleryItem[]> {
  try {
    const res = await fetch(url, { cache: 'no-cache' });
    if (!res.ok) return [];
    return parseFile((await res.json()) as GalleryFile);
  } catch {
    return [];
  }
}

async function loadFromCache(): Promise<GalleryItem[] | null> {
  const file = await readGalleryCacheFs();
  if (!file?.items.length) return null;
  const parsed = parseFile({ items: file.items as RawItem[] });
  return parsed.length > 0 ? parsed : null;
}

async function fetchRemote(): Promise<GalleryItem[]> {
  const remote = await fetchUrl(GALLERY_URL);
  if (remote.length > 0) {
    void writeGalleryCacheFs(remote);
    return remote;
  }
  const cached = await loadFromCache();
  if (cached?.length) return cached;
  return fetchUrl(GALLERY_FALLBACK_URL);
}

/** Load gallery (cached in memory until refresh). */
export async function loadGallery(): Promise<GalleryItem[]> {
  if (gallery) return gallery;
  gallery = await fetchRemote();
  return gallery;
}

/** Re-fetch from CDN (stale-while-revalidate when picker opens). */
export async function refreshGallery(): Promise<GalleryItem[]> {
  gallery = await fetchRemote();
  return gallery;
}
