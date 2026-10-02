import { describe, expect, it } from 'vitest';
import { compressMedia, scaledSize } from './compressMedia';

describe('compressMedia', () => {
  it('scales the longest edge down and never upscales', () => {
    expect(scaledSize(4000, 2000, 1000)).toEqual({ width: 1000, height: 500 });
    expect(scaledSize(800, 600, 1920)).toEqual({ width: 800, height: 600 });
  });

  it('keeps vector, animated and non-media files untouched', async () => {
    for (const type of ['image/svg+xml', 'image/gif', 'application/pdf']) {
      const file = new File(['x'], 'asset', { type });
      expect(await compressMedia(file)).toBe(file);
    }
  });

  it('falls back to the original when the browser cannot encode', async () => {
    const file = new File(['not really a png'], 'logo.png', { type: 'image/png' });
    expect(await compressMedia(file)).toBe(file);
  });
});
