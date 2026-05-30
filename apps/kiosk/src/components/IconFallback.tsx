export type FallbackIconKind = 'browser' | 'game';

const BROWSER_NAME_HINTS = [
  'chrome',
  'edge',
  'firefox',
  'browser',
  'opera',
  'brave',
  'safari',
  'internet',
];

/**
 * Pick a default glyph when the backend has no logo/thumbnail for an entry:
 * browser-like names get the globe, everything else the gamepad. Mirrors the
 * MUI `defaultIconKind` in `@gaming-cafe/ui` (kiosk runs without MUI).
 */
export function defaultIconKind(name: string): FallbackIconKind {
  const n = name.toLowerCase();
  return BROWSER_NAME_HINTS.some((hint) => n.includes(hint)) ? 'browser' : 'game';
}

interface IconFallbackProps {
  kind?: FallbackIconKind;
  name?: string;
  size?: number;
  className?: string;
}

/** Default placeholder icon shown when no backend art is available. */
export function IconFallback({ kind, name, size = 48, className }: IconFallbackProps) {
  const resolved = kind ?? (name ? defaultIconKind(name) : 'game');
  const common = {
    viewBox: '0 0 24 24',
    width: size,
    height: size,
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 1.6,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    className,
  };

  if (resolved === 'browser') {
    return (
      <svg {...common} role="img" aria-label="Browser">
        <circle cx="12" cy="12" r="9" />
        <ellipse cx="12" cy="12" rx="4" ry="9" />
        <path d="M3 9h18M3 15h18" />
      </svg>
    );
  }

  return (
    <svg {...common} role="img" aria-label="Game">
      <path d="M6 12h4M8 10v4" />
      <circle cx="15.5" cy="11.5" r="1" />
      <circle cx="17.5" cy="13.5" r="1" />
      <path d="M17.5 6h-11A4.5 4.5 0 0 0 2 10.5l1 6A2.5 2.5 0 0 0 7.4 17l1-1.5h7.2l1 1.5a2.5 2.5 0 0 0 4.4-.5l1-6A4.5 4.5 0 0 0 17.5 6Z" />
    </svg>
  );
}
