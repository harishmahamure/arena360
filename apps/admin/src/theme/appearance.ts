export interface Appearance {
  mode: 'light' | 'dark' | 'system';
  accent: string;
  density: 'comfortable' | 'compact';
  corners: 'rounded' | 'square';
}
export const defaultAppearance: Appearance = {
  mode: 'system',
  accent: '#176b51',
  density: 'comfortable',
  corners: 'rounded',
};
export function parseAppearance(value: unknown): Appearance {
  const input = value && typeof value === 'object' ? (value as Partial<Appearance>) : {};
  return {
    mode: input.mode === 'light' || input.mode === 'dark' ? input.mode : 'system',
    accent:
      typeof input.accent === 'string' && /^#[0-9a-f]{6}$/i.test(input.accent)
        ? input.accent
        : defaultAppearance.accent,
    density: input.density === 'compact' ? 'compact' : 'comfortable',
    corners: input.corners === 'square' ? 'square' : 'rounded',
  };
}
