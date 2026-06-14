import { describe, expect, it } from 'vitest';
import type { LaunchEntry } from './allowList';
import type { GalleryItem } from './gallery';
import {
  applyGalleryMediaToEntries,
  normalizeName,
  pickGalleryMedia,
  scoreGalleryMatch,
} from './galleryMatch';

const valorantEntry: LaunchEntry = {
  id: '1',
  name: 'Valorant',
  executablePath: 'C:\\Riot Games\\VALORANT\\live\\VALORANT.exe',
};

const galleryItems: GalleryItem[] = [
  {
    id: 'v-logo',
    kind: 'image',
    name: 'Valorant logo',
    url: 'https://cdn.example/logo.png',
  },
  {
    id: 'v-thumb',
    kind: 'image',
    name: 'Valorant poster',
    url: 'https://cdn.example/thumb.png',
  },
  {
    id: 'v-video',
    kind: 'video',
    name: 'Valorant trailer',
    url: 'https://cdn.example/trailer.mp4',
  },
];

describe('normalizeName', () => {
  it('lowercases and strips exe suffix', () => {
    expect(normalizeName('VALORANT.exe')).toBe('valorant');
  });
});

describe('scoreGalleryMatch', () => {
  it('prefers exact matches', () => {
    expect(scoreGalleryMatch('Valorant', 'Valorant poster')).toBeGreaterThan(
      scoreGalleryMatch('Valorant', 'League of Legends'),
    );
  });
});

describe('pickGalleryMedia', () => {
  it('assigns logo, thumbnail, and video for exact name family', () => {
    const patch = pickGalleryMedia(valorantEntry, galleryItems);
    expect(patch.logoUrl).toBe('https://cdn.example/logo.png');
    expect(patch.thumbnailUrl).toBe('https://cdn.example/thumb.png');
    expect(patch.videoUrl).toBe('https://cdn.example/trailer.mp4');
  });

  it('skips fields that are already set', () => {
    const patch = pickGalleryMedia(
      { ...valorantEntry, thumbnailUrl: 'https://cdn.example/custom.png' },
      galleryItems,
    );
    expect(patch.thumbnailUrl).toBeUndefined();
    expect(patch.logoUrl).toBe('https://cdn.example/logo.png');
  });

  it('returns empty patch when nothing matches', () => {
    const patch = pickGalleryMedia({ ...valorantEntry, name: 'Unknown Game XYZ' }, galleryItems);
    expect(Object.keys(patch)).toHaveLength(0);
  });
});

describe('applyGalleryMediaToEntries', () => {
  it('updates only entries with matches', () => {
    const entries = [
      valorantEntry,
      {
        id: '2',
        name: 'Not In Gallery',
        executablePath: 'C:\\Games\\other.exe',
      },
    ];
    const { entries: next, matched } = applyGalleryMediaToEntries(entries, galleryItems);
    expect(matched).toBe(1);
    expect(next[0]?.logoUrl).toBe('https://cdn.example/logo.png');
    expect(next[1]?.logoUrl).toBeUndefined();
  });
});
