import { Box, ButtonBase, Typography } from '@mui/material';
import { type FallbackIconKind, IconFallback } from './IconFallback';

export interface GameCardProps {
  name: string;
  thumbnailUrl?: string | null;
  logoUrl?: string | null;
  /** Forces the fallback glyph; otherwise derived from `name`. */
  fallbackKind?: FallbackIconKind;
  disabled?: boolean;
  /** Transient overlay label, e.g. "Launching...". */
  status?: string | null;
  title?: string;
  onClick?: () => void;
}

/**
 * ggCircuit-style game tile. Renders the backend thumbnail as cover art with
 * the logo overlaid at the bottom. When no backend art exists it degrades to a
 * slate gradient with the default {@link IconFallback} glyph and the name.
 */
export function GameCard({
  name,
  thumbnailUrl,
  logoUrl,
  fallbackKind,
  disabled,
  status,
  title,
  onClick,
}: GameCardProps) {
  return (
    <ButtonBase
      disabled={disabled}
      onClick={onClick}
      title={title ?? name}
      focusRipple
      sx={{
        position: 'relative',
        width: '100%',
        aspectRatio: '3 / 4',
        borderRadius: 2,
        overflow: 'hidden',
        border: '1px solid',
        borderColor: 'divider',
        bgcolor: 'background.paper',
        textAlign: 'left',
        transition: 'transform 0.12s ease, box-shadow 0.12s ease',
        '&:not(.Mui-disabled):hover': {
          transform: 'translateY(-2px)',
          boxShadow: '0 12px 32px rgba(0,0,0,0.45)',
        },
        '&.Mui-disabled': { opacity: 0.5 },
      }}
    >
      {thumbnailUrl ? (
        <Box
          component="img"
          src={thumbnailUrl}
          alt=""
          sx={{ position: 'absolute', inset: 0, width: '100%', height: '100%', objectFit: 'cover' }}
        />
      ) : (
        <Box
          sx={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            background: 'linear-gradient(135deg, #1C2436 0%, #141A2A 100%)',
          }}
        >
          <IconFallback name={name} kind={fallbackKind} size={56} sx={{ color: 'primary.main' }} />
        </Box>
      )}

      <Box
        sx={{
          position: 'absolute',
          inset: 'auto 0 0 0',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          p: 1.25,
          background: 'linear-gradient(180deg, rgba(11,15,26,0) 0%, rgba(11,15,26,0.85) 100%)',
        }}
      >
        {logoUrl ? (
          <Box
            component="img"
            src={logoUrl}
            alt=""
            sx={{ maxWidth: '80%', maxHeight: 48, objectFit: 'contain' }}
          />
        ) : (
          <Typography variant="subtitle2" sx={{ color: 'common.white', textAlign: 'center' }}>
            {name}
          </Typography>
        )}
      </Box>

      {status ? (
        <Box
          sx={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            bgcolor: 'rgba(11,15,26,0.6)',
            color: 'common.white',
            fontSize: '0.85rem',
          }}
        >
          {status}
        </Box>
      ) : null}
    </ButtonBase>
  );
}
