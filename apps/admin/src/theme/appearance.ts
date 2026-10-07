export type TextSize = 'small' | 'default' | 'large' | 'xlarge';
export interface Appearance {
  mode: 'light' | 'dark' | 'system';
  accent: string;
  density: 'comfortable' | 'compact';
  corners: 'rounded' | 'square';
  textSize: TextSize;
}
export const defaultAppearance: Appearance = {
  mode: 'system',
  accent: '#176b51',
  density: 'comfortable',
  corners: 'rounded',
  textSize: 'default',
};
export const textScale: Record<TextSize, number> = {
  small: 0.875,
  default: 1,
  large: 1.125,
  xlarge: 1.25,
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
    textSize:
      typeof input.textSize === 'string' && Object.hasOwn(textScale, input.textSize)
        ? input.textSize
        : defaultAppearance.textSize,
  };
}
