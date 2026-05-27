'use client';

import { Box, Paper, Typography } from '@mui/material';

export interface AuthLayoutProps {
  children: React.ReactNode;
  title?: string;
  description?: string;
  features?: string[];
  videoUrl?: string;
}

export default function AuthLayout({
  children,
  title = 'DualShock Arena Game Zone',
  videoUrl = 'https://assets.designtemplate.io/trailer_dualshock.webm',
}: AuthLayoutProps) {
  return (
    <Box
      sx={{
        minHeight: '100vh',
        display: 'flex',
        position: 'relative',
        overflow: 'hidden',
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
            top: 0,
            left: 0,
            width: '100%',
            height: '100%',
            objectFit: 'cover',
            zIndex: 0,
          }}
        >
          <source src={videoUrl} type="video/mp4" />
        </Box>
      )}

      <Box
        sx={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: `
              radial-gradient(circle at 20% 20%, rgba(255, 105, 0, 0.5) 0%, transparent 50%),
              radial-gradient(circle at 80% 80%, rgba(59, 130, 246, 0.05) 0%, transparent 50%),
              radial-gradient(circle at 50% 50%, rgba(139, 92, 246, 0.02) 0%, transparent 70%)
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
        <Paper
          elevation={0}
          sx={{
            width: '100%',
            maxWidth: 440,
            p: { xs: 3, sm: 4 },
            borderRadius: 3,
            border: '1px solid',
            borderColor: 'rgba(255, 255, 255, 0.1)',
            bgcolor: 'rgba(255, 255, 255, 0.05)',
            backdropFilter: 'blur(20px)',
            boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.1)',
            background: `rgba(255, 255, 255, 0.7)`,
          }}
        >
          {children}
        </Paper>

        <Typography variant="caption" color="white" sx={{ mt: 4, textAlign: 'center' }}>
          © 2025 {title}. All rights reserved.
        </Typography>
      </Box>
    </Box>
  );
}
