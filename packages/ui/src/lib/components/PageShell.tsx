'use client';

import { Box } from '@mui/material';
import type { ReactNode } from 'react';

export interface PageShellProps {
  children: ReactNode;
  maxWidth?: number | false;
  header?: ReactNode;
  toolbar?: ReactNode;
  footer?: ReactNode;
  /** Tighter vertical padding on md+ for dense list pages */
  dense?: boolean;
}

export function PageShell({
  children,
  maxWidth = false,
  header,
  toolbar,
  footer,
  dense = false,
}: PageShellProps) {
  return (
    <Box
      sx={{
        width: '100%',
        mx: 'auto',
        ...(maxWidth !== false ? { maxWidth } : {}),
        px: { xs: 2, md: 4 },
        py: { xs: 2, md: dense ? 2 : 3 },
      }}
    >
      {header}
      {toolbar ? <Box sx={{ mb: 2 }}>{toolbar}</Box> : null}
      {children}
      {footer ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>{footer}</Box>
      ) : null}
    </Box>
  );
}
