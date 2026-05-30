import { Box, type SxProps, type Theme } from '@mui/material';
import {
  defaultIconKind,
  type FallbackIconKind,
  IconFallback as IconFallbackPrimitive,
} from '../primitives';

export { defaultIconKind, type FallbackIconKind };

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
      <IconFallbackPrimitive kind={resolved} size={size} />
    </Box>
  );
}
