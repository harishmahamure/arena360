import type { LaunchEntry } from './allowList';
import type { GalleryItem } from './gallery';

/** Normalize a display name for fuzzy gallery matching. */
export function normalizeName(name: string): string {
  return name
    .toLowerCase()
    .replace(/\.(exe|bat|cmd|com|lnk)$/i, '')
    .replace(/[_-]+/g, ' ')
    .replace(/[^\w\s]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function tokenSet(name: string): Set<string> {
  return new Set(normalizeName(name).split(' ').filter(Boolean));
}

/** Higher score = better match between an allow-list entry and a gallery item. */
export function scoreGalleryMatch(entryName: string, galleryName: string): number {
  const entryNorm = normalizeName(entryName);
  const galleryNorm = normalizeName(galleryName);
  if (!entryNorm || !galleryNorm) return 0;
  if (entryNorm === galleryNorm) return 100;
  if (galleryNorm.includes(entryNorm) || entryNorm.includes(galleryNorm)) return 80;

  const entryTokens = tokenSet(entryName);
  const galleryTokens = tokenSet(galleryName);
  if (entryTokens.size === 0 || galleryTokens.size === 0) return 0;

  let overlap = 0;
  for (const token of entryTokens) {
    if (galleryTokens.has(token)) overlap += 1;
  }
  if (overlap === 0) return 0;
  return 40 + Math.round((overlap / entryTokens.size) * 30);
}

const LOGO_HINTS = ['logo', 'brand', 'wordmark'];
const THUMB_HINTS = ['thumb', 'thumbnail', 'poster', 'cover', 'card', 'hero'];

function roleBonus(field: 'logoUrl' | 'thumbnailUrl', galleryName: string): number {
  const norm = normalizeName(galleryName);
  const hints = field === 'logoUrl' ? LOGO_HINTS : THUMB_HINTS;
  return hints.some((h) => norm.includes(h)) ? 15 : 0;
}

function bestImageForField(
  entryName: string,
  items: GalleryItem[],
  field: 'logoUrl' | 'thumbnailUrl',
): string | null {
  let bestUrl: string | null = null;
  let bestScore = 0;
  for (const item of items) {
    if (item.kind !== 'image') continue;
    const score = scoreGalleryMatch(entryName, item.name) + roleBonus(field, item.name);
    if (score > bestScore) {
      bestScore = score;
      bestUrl = item.url;
    }
  }
  return bestScore >= 40 ? bestUrl : null;
}

function bestVideo(entryName: string, items: GalleryItem[]): string | null {
  let bestUrl: string | null = null;
  let bestScore = 0;
  for (const item of items) {
    if (item.kind !== 'video') continue;
    const score = scoreGalleryMatch(entryName, item.name);
    if (score > bestScore) {
      bestScore = score;
      bestUrl = item.url;
    }
  }
  return bestScore >= 40 ? bestUrl : null;
}

/** Fill empty media fields on an entry from gallery items (never overwrites existing URLs). */
export function pickGalleryMedia(entry: LaunchEntry, items: GalleryItem[]): Partial<LaunchEntry> {
  const patch: Partial<LaunchEntry> = {};
  if (!entry.logoUrl) {
    const logo = bestImageForField(entry.name, items, 'logoUrl');
    if (logo) patch.logoUrl = logo;
  }
  if (!entry.thumbnailUrl) {
    const thumb = bestImageForField(entry.name, items, 'thumbnailUrl');
    if (thumb) patch.thumbnailUrl = thumb;
  }
  if (!entry.videoUrl) {
    const video = bestVideo(entry.name, items);
    if (video) patch.videoUrl = video;
  }
  return patch;
}

export interface ApplyGalleryMediaResult {
  entries: LaunchEntry[];
  matched: number;
}

/** Apply gallery media to all entries; returns updated list and match count. */
export function applyGalleryMediaToEntries(
  entries: LaunchEntry[],
  items: GalleryItem[],
): ApplyGalleryMediaResult {
  if (items.length === 0) return { entries, matched: 0 };
  let matched = 0;
  const next = entries.map((entry) => {
    const patch = pickGalleryMedia(entry, items);
    if (Object.keys(patch).length === 0) return entry;
    matched += 1;
    return { ...entry, ...patch };
  });
  return { entries: next, matched };
}
