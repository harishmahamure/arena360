import { BrowserGlyph, defaultIconKind, type FallbackIconKind, GamepadGlyph } from './icons';

export type { FallbackIconKind };
export { defaultIconKind };

export interface IconFallbackPrimitiveProps {
  /** Explicit glyph; when omitted it is derived from `name`. */
  kind?: FallbackIconKind;
  /** Used to auto-pick the glyph when `kind` is omitted. */
  name?: string;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

/** Framework-neutral default placeholder icon for missing backend art. */
export function IconFallback({
  kind,
  name,
  size = 48,
  className,
  style,
}: IconFallbackPrimitiveProps) {
  const resolved = kind ?? (name ? defaultIconKind(name) : 'game');
  const title = resolved === 'browser' ? 'Browser' : 'Game';
  return resolved === 'browser' ? (
    <BrowserGlyph size={size} className={className} style={style} title={title} />
  ) : (
    <GamepadGlyph size={size} className={className} style={style} title={title} />
  );
}
