import { GALLERY_URL } from './config';

/**
 * Read-only media gallery sourced from a centrally hosted CDN JSON file.
 * Online only — each fetch goes to the CDN; no app-data cache or bundled fallback.
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

/** Unique URL so CDN Cache-Control and the webview HTTP cache are never used. */
function galleryJsonUrl(): string {
  const url = new URL(GALLERY_URL);
  url.searchParams.set('_', Date.now().toString(36));
  return url.href;
}

/** Cache-bust a gallery media URL for picker previews (does not persist). */
export function galleryMediaUrl(url: string, nonce: string): string {
  try {
    const parsed = new URL(url);
    parsed.searchParams.set('_', nonce);
    return parsed.href;
  } catch {
    return url;
  }
}

/** Fetch gallery from CDN (requires network; never cached or saved). */
export async function refreshGallery(): Promise<GalleryItem[]> {
  const res = await fetch(galleryJsonUrl(), { cache: 'no-store' });
  if (!res.ok) {
    throw new Error(`Gallery unavailable (${res.status})`);
  }
  return parseFile((await res.json()) as GalleryFile);
}
