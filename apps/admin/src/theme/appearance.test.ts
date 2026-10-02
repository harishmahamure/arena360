import { getContrastRatio } from '@mui/material/styles';
import { describe, expect, it } from 'vitest';
import { createAdminTheme } from './adminTheme';
import { defaultAppearance, parseAppearance } from './appearance';

describe('custom appearance', () => {
  it('sanitizes corrupted storage and invalid color values', () => {
    expect(parseAppearance({ accent: 'url(evil)', mode: 'unknown', density: 'tiny' })).toEqual(
      defaultAppearance,
    );
    expect(parseAppearance(null)).toEqual(defaultAppearance);
    expect(parseAppearance({ textSize: 'toString' }).textSize).toBe('default');
    expect(parseAppearance({ textSize: 'large' }).textSize).toBe('large');
  });
  it('builds dark and compact surfaces and legible custom accent buttons', () => {
    for (const accent of ['#ffffff', '#000000', '#365edc', '#ffbb00']) {
      const theme = createAdminTheme(
        { ...defaultAppearance, accent, density: 'compact', corners: 'square' },
        'dark',
      );
      expect(theme.palette.mode).toBe('dark');
      expect(theme.shape.borderRadius).toBe(2);
      expect(
        getContrastRatio(theme.palette.primary.main, theme.palette.primary.contrastText),
      ).toBeGreaterThanOrEqual(4.5);
    }
  });
});
