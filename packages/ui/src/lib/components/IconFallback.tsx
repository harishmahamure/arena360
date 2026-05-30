import { Box, type SxProps, type Theme } from '@mui/material';

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
 * Pick a sensible default icon when the backend has no logo/thumbnail for an
 * entry: browser-like names get the globe glyph, everything else the gamepad.
 */
export function defaultIconKind(name: string): FallbackIconKind {
  const n = name.toLowerCase();
  return BROWSER_NAME_HINTS.some((hint) => n.includes(hint)) ? 'browser' : 'game';
}

function BrowserGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="100%"
      height="100%"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      role="img"
      aria-label="Browser"
    >
      <circle cx="12" cy="12" r="9" />
      <ellipse cx="12" cy="12" rx="4" ry="9" />
      <path d="M3 9h18M3 15h18" />
    </svg>
  );
}

function GamepadGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="100%"
      height="100%"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      role="img"
      aria-label="Game"
    >
      <path d="M6 12h4M8 10v4" />
      <circle cx="15.5" cy="11.5" r="1" />
      <circle cx="17.5" cy="13.5" r="1" />
      <path d="M17.5 6h-11A4.5 4.5 0 0 0 2 10.5l1 6A2.5 2.5 0 0 0 7.4 17l1-1.5h7.2l1 1.5a2.5 2.5 0 0 0 4.4-.5l1-6A4.5 4.5 0 0 0 17.5 6Z" />
    </svg>
  );
}

export interface IconFallbackProps {
  /** Explicit glyph; when omitted it is derived from `name`. */
  kind?: FallbackIconKind;
  /** Used to auto-pick the glyph when `kind` is omitted. */
  name?: string;
  size?: number;
  sx?: SxProps<Theme>;
}

/**
 * Default placeholder icon (browser globe or gamepad) shown when no backend
 * art is available for a game/launch entry.
 */
export function IconFallback({ kind, name, size = 48, sx }: IconFallbackProps) {
  const resolved = kind ?? (name ? defaultIconKind(name) : 'game');
  return (
    <Box
      sx={{
        width: size,
        height: size,
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        color: 'text.secondary',
        ...sx,
      }}
    >
      {resolved === 'browser' ? <BrowserGlyph /> : <GamepadGlyph />}
    </Box>
  );
}
