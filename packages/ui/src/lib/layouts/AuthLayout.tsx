'use client';

import { Box, Paper, Typography } from '@mui/material';

export interface AuthLayoutProps {
  children: React.ReactNode;
  /** Product/brand name, used in the default footer. */
  title?: string;
  description?: string;
  features?: string[];
  /** Looped, muted background video URL. */
  videoUrl?: string;
  /** Optional brand slot rendered above the card (logo, station name). */
  brandSlot?: React.ReactNode;
  /** Optional footer override; defaults to a copyright line. */
  footer?: React.ReactNode;
}

const DEFAULT_VIDEO = 'https://assets.designtemplate.io/trailer_dualshock.webm';

/**
 * Dark, ggCircuit-style authentication shell: a looped video background with a
 * frosted slate glass card centered on top. Shared by the admin login and the
 * kiosk player login so both surfaces match. The orange accent comes through
 * the surrounding MUI theme (admin wraps this in `darkTheme`).
 */
export default function AuthLayout({
  children,
  title = 'Arena360',
  videoUrl = DEFAULT_VIDEO,
  brandSlot,
  footer,
}: AuthLayoutProps) {
  return (
    <Box
      sx={{
        minHeight: '100vh',
        display: 'flex',
        position: 'relative',
        overflow: 'hidden',
        bgcolor: '#0B0F1A',
      }}
    >
      {videoUrl && (
        <Box
          component="video"
          autoPlay
          loop
          muted
          playsInline
          sx={{
            position: 'absolute',
            inset: 0,
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            zIndex: 0,
          }}
        >
          <source src={videoUrl} type="video/mp4" />
          <source src={videoUrl} type="video/webm" />
        </Box>
      )}

      {/* Scrim + orange glow so the card stays legible over any footage. */}
      <Box
        sx={{
          position: 'absolute',
          inset: 0,
          background: `
              radial-gradient(circle at 20% 20%, rgba(255, 105, 0, 0.28) 0%, transparent 45%),
              radial-gradient(circle at 80% 80%, rgba(59, 130, 246, 0.16) 0%, transparent 45%),
              linear-gradient(180deg, rgba(11, 15, 26, 0.72) 0%, rgba(11, 15, 26, 0.88) 100%)
            `,
          pointerEvents: 'none',
          zIndex: 1,
        }}
      />

      <Box
        sx={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          alignItems: 'center',
          p: { xs: 3, sm: 6 },
          position: 'relative',
          textAlign: 'center',
          zIndex: 2,
        }}
      >
        {brandSlot ? <Box sx={{ mb: 3 }}>{brandSlot}</Box> : null}

        <Paper
          elevation={0}
          sx={{
            width: '100%',
            maxWidth: 440,
            p: { xs: 3, sm: 4 },
            borderRadius: 3,
            border: '1px solid',
            borderColor: 'rgba(255, 255, 255, 0.08)',
            bgcolor: 'rgba(20, 26, 42, 0.72)',
            backdropFilter: 'blur(20px)',
            boxShadow: '0 24px 64px 0 rgba(0, 0, 0, 0.55)',
            color: 'common.white',
            textAlign: 'left',
          }}
        >
          {children}
        </Paper>

        {footer ?? (
          <Typography
            variant="caption"
            sx={{ mt: 4, textAlign: 'center', color: 'rgba(232, 236, 244, 0.7)' }}
          >
            © {new Date().getFullYear()} {title}. All rights reserved.
          </Typography>
        )}
      </Box>
    </Box>
  );
}
