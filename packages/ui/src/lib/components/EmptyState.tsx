'use client';

import { InboxOutlined } from '@mui/icons-material';
import { Box, Button, Paper, Typography, useTheme } from '@mui/material';
import type { ReactNode } from 'react';

export interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  actionLabel?: string;
  onAction?: () => void;
}

export function EmptyState({ icon, title, description, actionLabel, onAction }: EmptyStateProps) {
  const theme = useTheme();
  const showAction = actionLabel && onAction;

  return (
    <Paper
      elevation={0}
      sx={{
        border: `1px solid ${theme.palette.divider}`,
        borderRadius: 1,
        py: 6,
        px: 3,
      }}
    >
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          textAlign: 'center',
          gap: 1.5,
        }}
      >
        <Box sx={{ color: 'text.secondary', display: 'flex' }}>
          {icon ?? <InboxOutlined sx={{ fontSize: 48 }} />}
        </Box>
        <Typography variant="h6" fontWeight={600}>
          {title}
        </Typography>
        {description && (
          <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 400 }}>
            {description}
          </Typography>
        )}
        {showAction && (
          <Button variant="contained" onClick={onAction} sx={{ mt: 1 }}>
            {actionLabel}
          </Button>
        )}
      </Box>
    </Paper>
  );
}
